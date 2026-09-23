use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExportDeclaration, ParsedSource, ParsedStatement};
use surge_ts_types::{FunctionType, ProgramTypeStore, with_program_type_store};

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.
pub(crate) use crate::metrics::*;

use crate::context::{CheckerContext, CheckerOptions, CompatibilityStats, FileKind};
use crate::default_lib::load_generated_default_lib_inputs;
use crate::driver::sync_global_this_symbol;
use crate::modules::{ModuleExportTable, ModuleImportBindings, resolve_module_export_tables};
use crate::paths::canonicalize_if_exists_string;
use crate::symbols::{SymbolTable, TypeDeclarationScope, TypeDeclarationTable};

mod ambient;
pub(crate) mod binding;
mod check_files;
mod classes;
mod forward_references;
mod heritage;
mod index_constraints;
mod namespaces;
mod property_initialization;
pub(crate) mod diagnostics;
mod file_classify;
mod globals;
mod parse;
mod phase;
mod probes;
mod schedule;
mod statements;
mod unused_locals;

pub(crate) use ambient::*;
pub(crate) use binding::*;
use check_files::*;
pub(crate) use check_files::{emit_grammar_diagnostics, unclaimed_parser_errors};
pub(crate) use classes::*;
pub(crate) use diagnostics::*;
pub(crate) use file_classify::*;
pub(crate) use globals::*;
pub(crate) use namespaces::{
    MEANING_NAMESPACE, MEANING_TYPE, MEANING_VALUE, NamespaceInfo, NamespaceRegistry,
    is_instantiated_namespace,
};
use parse::*;
pub(crate) use phase::*;
pub(crate) use probes::*;
pub(crate) use statements::*;
pub(crate) use unused_locals::report_unused_declaration_list;

#[derive(Debug, Clone)]
pub struct SourceFileInput {
    pub file_name: String,
    pub source_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FunctionDeclarationLocation {
    pub(crate) file_index: usize,
    pub(crate) statement_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedProgramFile {
    pub(crate) file_name: String,
    /// Precomputed `source_text.contains("export default")`. The full source text
    /// is dropped after parsing — it is only consumed by this textual heuristic
    /// (diagnostic rendering uses the CLI's separate source map), and retaining a
    /// copy of every dependency `.d.ts` here was a sizeable share of peak RSS.
    pub(crate) has_export_default: bool,
    /// Precomputed `source_text.contains("typeof")`. A `typeof` TYPE node can
    /// only come from the keyword, so `false` proves the parse tree contains no
    /// typeof type — the module-local-values stage skips such files' tables.
    /// Identifier substrings (`typeofFoo`) only cause a harmless eager build.
    pub(crate) contains_typeof: bool,
    pub(crate) statements: Vec<ParsedStatement>,
    pub(crate) parser_errors: Vec<surge_ts_syntax::ParserError>,
    pub(crate) is_module: bool,
    /// Specifiers written as `import("…")` in type positions (see
    /// [`surge_ts_syntax::ParsedSource::import_call_specifiers`]).
    pub(crate) import_call_specifiers: Vec<String>,
    pub(crate) file_kind: FileKind,
    /// Module-wide identifier reads (see [`surge_ts_syntax::ParsedSource`]),
    /// retained only when `noUnusedLocals` is enabled; empty otherwise.
    pub(crate) module_reads: Vec<String>,
    /// See [`surge_ts_syntax::ParsedSource::definite_writes`].
    pub(crate) definite_writes: Vec<String>,
    /// Byte ranges of the lines an `@ts-expect-error`/`@ts-ignore` directive
    /// suppresses (see [`surge_ts_syntax::ParsedSource::suppressed_ranges`]).
    pub(crate) suppressed_ranges: Vec<surge_ts_syntax::TextSpan>,
    /// Grammar findings from the parser's AST walk (see
    /// [`surge_ts_syntax::ParsedSource::grammar_diagnostics`]), turned into
    /// diagnostics at the start of the file's check.
    pub(crate) grammar_diagnostics: Vec<surge_ts_syntax::ParsedGrammarDiagnostic>,
    /// See [`surge_ts_syntax::ParsedSource::parenthesized_expressions`]; handed
    /// to the context for the file's check.
    pub(crate) parenthesized_expressions: std::sync::Arc<[surge_ts_syntax::ParenthesizedExpressionSpan]>,
    /// See [`surge_ts_syntax::ParsedSource::let_assignments`].
    pub(crate) let_assignments: std::sync::Arc<[surge_ts_syntax::LetAssignmentSummary]>,
    /// See [`surge_ts_syntax::ParsedSource::json_module_type`]. Set for every
    /// `.json` file; its export table is built from this instead of from
    /// `statements`, which are always empty for such a file.
    pub(crate) json_module_type: Option<surge_ts_syntax::ParsedType>,
    /// See [`surge_ts_syntax::ParsedSource::jsx_factory_uses`].
    pub(crate) jsx_factory_uses: surge_ts_syntax::JsxFactoryUses,
}

#[derive(Debug, Clone)]
pub struct ProgramCheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub stats: CompatibilityStats,
    /// A program file failed to parse the way tsc's parser fails, so
    /// `diagnostics` holds only syntactic diagnostics (tsc then reports no
    /// program or semantic diagnostics either).
    pub syntax_errors: bool,
}

#[derive(Debug, Clone)]
struct ProgramCheckSharedState {
    /// Prebuilt global+ambient declaration table for script (non-module) files.
    /// Built once on the main thread before the check-phase fan-out; workers
    /// clone it per file rather than rebuilding it, so every script file sees
    /// the same merged global interfaces.
    script_type_declarations: TypeDeclarationTable,
    global_symbols: SymbolTable,
    /// Each script's own top-level values, indexed by file; see
    /// [`globals::collect_script_values`].
    script_values: Vec<Option<Arc<SymbolTable>>>,
    function_signatures: HashMap<FunctionDeclarationLocation, FunctionType>,
    module_analyses: Vec<Option<ModuleAnalysis>>,
    module_import_bindings: Vec<Option<ModuleImportBindings>>,
    module_resolution_scopes: Vec<Option<Arc<TypeDeclarationScope>>>,
}

#[derive(Debug)]
struct FileCheckResult {
    file_index: usize,
    diagnostics: Vec<Diagnostic>,
    stats: CompatibilityStats,
}

#[derive(Debug, Clone)]
pub(crate) struct ModuleAnalysis {
    local_type_declarations: Arc<TypeDeclarationTable>,
    local_symbols: SymbolTable,
    local_symbol_names: Option<Arc<surge_ts_types::fx::FxHashSet<Arc<str>>>>,
    local_export_table: ModuleExportTable,
}

impl ModuleAnalysis {
    pub(crate) fn local_type_declarations(&self) -> &Arc<TypeDeclarationTable> {
        &self.local_type_declarations
    }

    pub(crate) fn local_symbols(&self) -> &SymbolTable {
        &self.local_symbols
    }

    fn has_local_symbol(&self, name: &str) -> bool {
        self.local_symbol_names.as_ref().map_or_else(
            || self.local_symbols.get(name).is_some(),
            |names| names.contains(name),
        )
    }

    fn release_local_symbols_to_names(&mut self) {
        let names = self
            .local_symbols
            .iter_shared()
            .map(|(name, _)| name.clone())
            .collect();
        self.local_symbol_names = Some(Arc::new(names));
        self.local_symbols = SymbolTable::new();
    }

    pub(crate) fn local_export_table(&self) -> &ModuleExportTable {
        &self.local_export_table
    }
}

// --- Temporary SURGE_EQ_STATS probe: measures how many files' preliminary and
// final module analyses are already output-equal (the ceiling for any
// equality-based final-round skip) and how much preliminary time they carry.
// Remove once the skip decision is recorded.

pub fn check_program(files: Vec<SourceFileInput>) -> Vec<Diagnostic> {
    check_program_with_options(files, CheckerOptions::default())
}

pub fn check_program_with_options(
    files: Vec<SourceFileInput>,
    options: CheckerOptions,
) -> Vec<Diagnostic> {
    check_program_with_stats(files, options).diagnostics
}

pub fn check_program_with_stats(
    files: Vec<SourceFileInput>,
    options: CheckerOptions,
) -> ProgramCheckResult {
    check_program_with_stats_and_jobs(files, options, 1)
}

pub fn check_program_with_stats_and_jobs(
    files: Vec<SourceFileInput>,
    options: CheckerOptions,
    jobs: usize,
) -> ProgramCheckResult {
    check_program_with_prescanned_sources(files, Vec::new(), options, jobs)
}

/// Check a program whose files a loader has already parsed while building the
/// module graph. A prescanned entry replaces the program's own parse of the
/// file it names; anything unmatched is parsed here as usual. The parse is a
/// pure function of `(source text, file name)`, so reuse is only sound when
/// the caller parsed the same text this program is handed — which is why the
/// side channel is keyed by file name rather than trusting input order.
pub fn check_program_with_prescanned_sources(
    files: Vec<SourceFileInput>,
    prescanned: Vec<ParsedSource>,
    options: CheckerOptions,
    jobs: usize,
) -> ProgramCheckResult {
    let store = ProgramTypeStore::new();
    store.set_strict_null_checks(options.strict_null_checks);
    with_program_type_store(store.clone(), || {
        check_program_with_stats_and_jobs_inner(files, prescanned, options, jobs, store)
    })
}

struct ProgramRun {
    timings: Option<Arc<Mutex<ProgramTimings>>>,
    timings_enabled: bool,
    program_start: Instant,
    census_external: CensusExternalRetention,
    parsed_files: Vec<ParsedProgramFile>,
    ctx: CheckerContext,
}

struct GlobalCollection {
    global_symbols: SymbolTable,
    script_values: Vec<Option<Arc<SymbolTable>>>,
    function_signatures: HashMap<FunctionDeclarationLocation, FunctionType>,
    global_type_declarations: TypeDeclarationTable,
    type_declaration_collection_start: Instant,
}

struct PreliminaryPass {
    local_type_declarations_by_module: Vec<Option<Arc<TypeDeclarationTable>>>,
    preliminary_module_import_bindings: Vec<Option<ModuleImportBindings>>,
    preliminary_module_resolution_scopes: Vec<Option<Arc<TypeDeclarationScope>>>,
    analysis_worker_count: usize,
    preliminary_module_analyses: Vec<Option<ModuleAnalysis>>,
}

/// State that outlives the final module-analysis round: the last binding
/// generation plus the preliminary structures the final import binding still
/// falls back to.
struct ModuleBinding {
    module_binding_start: Instant,
    module_export_tables: Vec<Option<ModuleExportTable>>,
    module_import_bindings: Vec<Option<ModuleImportBindings>>,
    module_resolution_scopes: Vec<Option<Arc<TypeDeclarationScope>>>,
    module_analyses: Vec<Option<ModuleAnalysis>>,
    local_type_declarations_by_module: Vec<Option<Arc<TypeDeclarationTable>>>,
    preliminary_module_import_bindings: Vec<Option<ModuleImportBindings>>,
}

fn check_program_with_stats_and_jobs_inner(
    files: Vec<SourceFileInput>,
    prescanned: Vec<ParsedSource>,
    options: CheckerOptions,
    jobs: usize,
    store: Arc<ProgramTypeStore>,
) -> ProgramCheckResult {
    if files.is_empty() {
        return ProgramCheckResult {
            diagnostics: Vec::new(),
            stats: CompatibilityStats::default(),
            syntax_errors: false,
        };
    }

    let ProgramRun {
        timings,
        timings_enabled,
        program_start,
        mut census_external,
        mut parsed_files,
        mut ctx,
    } = start_program_run(files, prescanned, options, jobs, &store);
    let syntax_errors = diagnostics::program_has_syntax_errors(&parsed_files);
    let globals = collect_program_globals(&parsed_files, &mut ctx, &timings, program_start);
    let preliminary = run_preliminary_pass(
        &mut parsed_files,
        &mut ctx,
        &timings,
        program_start,
        jobs,
        &store,
        &mut census_external,
        &globals.global_symbols,
    );
    let binding = bind_and_analyze_modules(
        &mut parsed_files,
        &mut ctx,
        &timings,
        program_start,
        &store,
        &mut census_external,
        &globals.global_symbols,
        globals.type_declaration_collection_start,
        preliminary,
    );
    let mut shared_state = finalize_module_bindings(
        &mut parsed_files,
        &mut ctx,
        &timings,
        program_start,
        binding,
        globals,
    );
    build_module_local_values(
        &parsed_files,
        &shared_state.module_analyses,
        &shared_state.module_import_bindings,
        &mut ctx,
    );
    record_rss_stage(
        timings.as_ref(),
        "module_local_values",
        program_start.elapsed(),
    );
    emit_check_phase_retention_census(
        "before_check_phase",
        &ctx,
        &store,
        &shared_state,
        &parsed_files,
    );
    release_declaration_asts(&mut parsed_files, &ctx, &timings, program_start);
    run_check_phase(
        &mut parsed_files,
        &mut shared_state,
        &mut ctx,
        &timings,
        program_start,
        jobs,
    );
    emit_check_phase_retention_census(
        "after_check_phase",
        &ctx,
        &store,
        &shared_state,
        &parsed_files,
    );
    // Checking is complete and the diagnostics are extracted: the cross-file
    // program state and every remaining parse tree are dead. Dropping them here
    // (rather than at function exit, after the finish measurements) makes the
    // finish footprint reflect what a long-lived host would actually retain.
    let skip_teardown = fast_process_exit()
        && timings.is_none()
        && !crate::metrics::rss_stages_enabled()
        && !crate::metrics::retention_census_enabled()
        && !type_graph_census_enabled();
    if skip_teardown {
        std::mem::forget(shared_state);
        std::mem::forget(parsed_files);
    } else {
        drop(shared_state);
        drop(parsed_files);
    }
    let mut result = finish_program_run(
        ctx,
        &store,
        &timings,
        timings_enabled,
        program_start,
        census_external,
        skip_teardown,
    );
    if syntax_errors {
        result.diagnostics.retain(diagnostics::is_syntactic_diagnostic);
        result.syntax_errors = true;
    }
    result
}

fn start_program_run(
    files: Vec<SourceFileInput>,
    prescanned: Vec<ParsedSource>,
    options: CheckerOptions,
    jobs: usize,
    store: &Arc<ProgramTypeStore>,
) -> ProgramRun {
    let mut files = files;
    inject_generated_default_lib_inputs(&mut files, options.no_lib);
    let source_text_bytes = files
        .iter()
        .map(|file| file.source_text.len() as u64)
        .sum::<u64>();

    let timings_enabled = std::env::var_os("SURGE_TIMINGS").is_some();
    // RSS stage sampling piggybacks on the timings carrier so `SURGE_RSS=1`
    // alone profiles memory without the full counter/timing report.
    let instrumentation_enabled = timings_enabled || std::env::var_os("SURGE_RSS").is_some();
    set_counters_enabled(timings_enabled);
    let timings = instrumentation_enabled.then(|| Arc::new(Mutex::new(ProgramTimings::default())));
    let program_start = Instant::now();
    record_rss_stage(timings.as_ref(), "start", program_start.elapsed());
    reset_program_counters();
    reset_dts_expansion_trace();
    crate::paths::clear_canonicalize_cache();
    crate::modules::clear_relative_module_cache();
    crate::modules::clear_star_export_unresolved_cache();
    crate::modules::clear_namespace_alias_table_cache();
    crate::modules::set_node_esm_files(
        options
            .node_module_resolution
            .then(|| Arc::new(options.esm_module_files.clone())),
    );
    crate::modules::set_allow_arbitrary_extensions(options.allow_arbitrary_extensions);

    let parse_start = Instant::now();
    let parsed_files = parse_program_files(files, prescanned, jobs, timings.as_ref());
    let ast_nodes = parsed_files
        .iter()
        .map(|file| file.statements.len() as u64)
        .sum::<u64>();
    let census_external = CensusExternalRetention {
        ast_nodes,
        ast_estimated_bytes: ast_nodes * std::mem::size_of::<ParsedStatement>() as u64,
        source_text_bytes,
        ..CensusExternalRetention::default()
    };
    record_program_timing(timings.as_ref(), |timings| {
        timings.parsing += parse_start.elapsed()
    });
    record_rss_stage(timings.as_ref(), "parsing", program_start.elapsed());
    if let Some(path) = std::env::var_os("SURGE_FILE_ORDER_DUMP") {
        let mut out = String::new();
        for (index, file) in parsed_files.iter().enumerate() {
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "{index}\t{:?}\t{}\t{}",
                file.file_kind, file.is_module, file.file_name
            );
        }
        let _ = std::fs::write(path, out);
    }
    let module_file_index_by_identity = parsed_files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            (
                canonicalize_if_exists_string(std::path::Path::new(&file.file_name)).into(),
                index,
            )
        })
        .collect::<surge_ts_types::fx::FxHashMap<Arc<str>, usize>>();
    let file_kinds = parsed_files
        .iter()
        .map(|file| (file.file_name.clone(), file.file_kind))
        .collect::<surge_ts_types::fx::FxHashMap<_, _>>();
    let first_file_name = parsed_files
        .first()
        .map(|file| file.file_name.clone())
        .unwrap_or_default();
    let mut ctx = CheckerContext::new(first_file_name, options, file_kinds);
    ctx.declaration_environment_store.mark_program_lifetime();
    ctx.timings = timings.clone();
    ctx.set_module_file_index_by_identity(module_file_index_by_identity);
    emit_type_graph_census("after_loading_parsing", Some(&ctx), &store, census_external);
    ProgramRun {
        timings,
        timings_enabled,
        program_start,
        census_external,
        parsed_files,
        ctx,
    }
}

fn collect_program_globals(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: &Option<Arc<Mutex<ProgramTimings>>>,
    program_start: Instant,
) -> GlobalCollection {
    let mut global_symbols = SymbolTable::new();
    let mut function_signatures = HashMap::new();

    let ambient_collection_start = Instant::now();
    emit_parser_diagnostics(&parsed_files, ctx);
    ctx.begin_resolution_stage();
    // Three ordered steps, and the order is load-bearing in both directions.
    //
    // Ambient global *types* merge first so the ambient declaration is the merge
    // base: a merged interface takes its declaring file and resolution scope
    // from whichever fragment merged first, and only the ambient one can resolve
    // the member annotations.
    //
    // `declare global` block types merge second, before any value is lowered:
    // @types/node's `globals.d.ts` (a script) declares `var process:
    // NodeJS.Process` while the interface's members are re-opened from
    // `process.d.ts` inside a `declare global`, so lowering the value first
    // froze `process` against whatever partial `NodeJS.Process` existed.
    //
    // Ambient *values* lower last, against the fully merged table.
    collect_ambient_global_types(&parsed_files, ctx, timings.as_ref());
    crate::driver::collect_global_augmentations(&parsed_files, ctx);
    lower_ambient_global_values(&parsed_files, ctx);
    collect_umd_global_names(&parsed_files, ctx);
    namespaces::collect_namespace_registry(&parsed_files, ctx);
    collect_ambient_modules(&parsed_files, ctx, timings.as_ref());
    record_program_timing(timings.as_ref(), |timings| {
        timings.ambient_collection += ambient_collection_start.elapsed()
    });
    record_rss_stage(
        timings.as_ref(),
        "ambient_collection",
        program_start.elapsed(),
    );

    let type_declaration_collection_start = Instant::now();
    collect_global_type_declarations(&parsed_files, ctx, timings.as_ref());
    record_program_timing(timings.as_ref(), |timings| {
        timings.root_source_global_collection += type_declaration_collection_start.elapsed()
    });
    let mut global_type_declarations = clone_type_declaration_table(
        &ctx.type_declarations,
        timings.as_ref(),
        TableCloneKind::General,
    );
    // Same merge as `script_type_declarations` below, applied once here so every
    // consumer of the global table sees the fully merged global interfaces rather
    // than shadowing them with a single contributor's declaration.
    crate::symbols::merge_shared_table_into(
        &mut global_type_declarations,
        ctx.ambient_global_type_declarations.as_ref(),
    );
    collect_global_function_signatures(
        &parsed_files,
        &mut global_symbols,
        &mut function_signatures,
        ctx,
    );
    collect_global_variables(&parsed_files, &mut global_symbols, ctx);
    let script_values = collect_script_values(&parsed_files, &global_symbols, ctx);
    record_rss_stage(
        timings.as_ref(),
        "global_collection",
        program_start.elapsed(),
    );
    GlobalCollection {
        global_symbols,
        script_values,
        function_signatures,
        global_type_declarations,
        type_declaration_collection_start,
    }
}

fn run_preliminary_pass(
    parsed_files: &mut Vec<ParsedProgramFile>,
    ctx: &mut CheckerContext,
    timings: &Option<Arc<Mutex<ProgramTimings>>>,
    program_start: Instant,
    jobs: usize,
    store: &Arc<ProgramTypeStore>,
    census_external: &mut CensusExternalRetention,
    global_symbols: &SymbolTable,
) -> PreliminaryPass {
    // PRELIMINARY PASS: collect types and resolve imports/exports to make them available for function signature collection
    let type_collection_start = Instant::now();
    ctx.begin_resolution_stage();
    let (
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
        preliminary_type_diagnostics,
    ) = collect_preliminary_module_type_bindings(&parsed_files, ctx, timings.as_ref());
    for diagnostic in preliminary_type_diagnostics {
        ctx.push(diagnostic);
    }
    if ctx.options.skip_lib_check && !eq_probe_enabled() {
        for parsed_file in parsed_files.iter_mut() {
            release_lowered_declaration_type_bodies(parsed_file);
        }
        crate::metrics::release_free_memory();
    }
    record_program_timing(timings.as_ref(), |timings| {
        timings.preliminary_module_type_binding_collection += type_collection_start.elapsed()
    });

    let scope_build_start = Instant::now();
    let preliminary_module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &preliminary_module_import_bindings,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });

    let type_collection_start = Instant::now();
    // Experimental (`SURGE_PARALLEL_ANALYSIS=1`): the preliminary pass never
    // lowers `declare global` values, so its only cross-module writes go
    // through the speculative cache sessions — but full byte-identity is still
    // blocked by declaration-environment identity: `DeclarationEnvironmentKey`
    // embeds context pointers, so the physical-interface caches key the same
    // logical instantiation differently across context instances, flipping
    // hit/miss on entries whose values are context-sensitive in a way conflict
    // validation cannot see (tRPC: 2 extra TS2304). Off by default until
    // environment identity is content-based; the serial-equivalent commit,
    // and per-worker contexts are in place.
    ctx.begin_resolution_stage();
    let analysis_worker_count = if std::env::var_os("SURGE_PARALLEL_ANALYSIS").is_some() {
        resolve_worker_count(jobs, &parsed_files)
    } else {
        1
    };
    let preliminary_module_analyses = if analysis_worker_count > 1 {
        collect_module_analyses_with_bindings_parallel(
            &parsed_files,
            &local_type_declarations_by_module,
            &preliminary_module_import_bindings,
            false,
            ctx,
            timings.as_ref(),
            analysis_worker_count,
        )
    } else {
        collect_module_analyses_with_bindings(
            parsed_files,
            &local_type_declarations_by_module,
            &preliminary_module_import_bindings,
            false,
            ctx,
            timings.as_ref(),
        )
    };
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_analysis_collection += type_collection_start.elapsed()
    });
    record_rss_stage(
        timings.as_ref(),
        "preliminary_module_analysis",
        program_start.elapsed(),
    );
    census_external.module_analysis_entries = preliminary_module_analyses
        .iter()
        .filter(|analysis| analysis.is_some())
        .count() as u64;
    census_external.module_analysis_estimated_bytes =
        census_external.module_analysis_entries * std::mem::size_of::<ModuleAnalysis>() as u64;
    emit_type_graph_census(
        "after_preliminary_analysis",
        Some(&ctx),
        &store,
        *census_external,
    );
    crate::metrics::emit_retention_census(
        "after_preliminary_analysis",
        Some(&ctx),
        &store,
        crate::metrics::RetentionCensusView {
            preliminary_module_analyses: Some(&preliminary_module_analyses),
            preliminary_module_import_bindings: Some(&preliminary_module_import_bindings),
            parsed_files: Some(&parsed_files),
            global_symbols: Some(&global_symbols),
            ..Default::default()
        },
    );
    PreliminaryPass {
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
        preliminary_module_resolution_scopes,
        analysis_worker_count,
        preliminary_module_analyses,
    }
}

fn bind_and_analyze_modules(
    parsed_files: &mut Vec<ParsedProgramFile>,
    ctx: &mut CheckerContext,
    timings: &Option<Arc<Mutex<ProgramTimings>>>,
    program_start: Instant,
    store: &Arc<ProgramTypeStore>,
    census_external: &mut CensusExternalRetention,
    global_symbols: &SymbolTable,
    type_declaration_collection_start: Instant,
    preliminary: PreliminaryPass,
) -> ModuleBinding {
    let PreliminaryPass {
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
        preliminary_module_resolution_scopes,
        analysis_worker_count,
        preliminary_module_analyses,
    } = preliminary;
    // A type published through a *value* export is resolved here, and a
    // `typeof <value>` inside it reaches for the declaring module's local value
    // table — which was only built after this whole pass, so the member died at
    // the sentinel while the rest of the instantiation survived
    // (`export const t = make<R>()` where the returned type has a `typeof f`
    // member). The preliminary analyses already hold everything that build
    // needs, so seed the map before publication; the authoritative build after
    // the final analysis round replaces it.
    if early_module_local_values_enabled() {
        build_module_local_values(
            parsed_files,
            &preliminary_module_analyses,
            &preliminary_module_import_bindings,
            ctx,
        );
    }
    let module_binding_start = Instant::now();
    let export_resolution_start = Instant::now();
    // Superseded binding rounds are reassigned (not shadowed) so each round's
    // tables free as soon as the next round replaces them, and the preliminary
    // structures are dropped at the `preliminary_release` boundary below —
    // otherwise every generation stays alive through the peak-RSS check phase.
    let mut module_export_tables = {
        let local_module_export_tables = preliminary_module_analyses
            .iter()
            .map(|analysis| {
                analysis
                    .as_ref()
                    .map(|analysis| analysis.local_export_table.clone())
            })
            .collect::<Vec<_>>();
        ctx.begin_resolution_stage();
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, ctx)
    };
    record_program_timing(timings.as_ref(), |timings| {
        timings.preliminary_export_table_resolution += export_resolution_start.elapsed()
    });
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_declaration_collection_start.elapsed()
    });
    let import_binding_start = Instant::now();
    let mut module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &preliminary_module_analyses,
        &module_export_tables,
        &preliminary_module_resolution_scopes,
        ctx,
    );
    drop(preliminary_module_resolution_scopes);
    record_program_timing(timings.as_ref(), |timings| {
        timings.import_binding_resolution += import_binding_start.elapsed()
    });
    let scope_build_start = Instant::now();
    let mut module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &module_import_bindings,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });
    let import_binding_start = Instant::now();
    // The rebuild reads only the analyses, export tables, and scopes — never
    // the previous bindings — so the superseded generation is dropped first
    // rather than held across the rebuild (two full binding generations at
    // once is a transient footprint hump on dependency-heavy projects).
    drop(std::mem::take(&mut module_import_bindings));
    module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &preliminary_module_analyses,
        &module_export_tables,
        &module_resolution_scopes,
        ctx,
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.import_binding_resolution += import_binding_start.elapsed()
    });
    let scope_build_start = Instant::now();
    drop(std::mem::take(&mut module_resolution_scopes));
    module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &module_import_bindings,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });
    // The preliminary analyses and the round-1 resolved export tables are dead
    // from here on (their last reads are the round-2 import bindings above);
    // both are re-derived from the final analyses below. Releasing them before
    // the final analysis round matters because that round re-materializes every
    // module's retained type surface, and holding two complete generations of
    // it simultaneously is the module-analysis RSS peak on dependency-heavy
    // projects. The `SURGE_EQ_STATS` probe keeps the preliminary analyses alive
    // to compare the two generations.
    let early_release_enabled =
        std::env::var("SURGE_PRELIM_EARLY_RELEASE").as_deref() != Ok("0") && !eq_probe_enabled();
    let preliminary_module_analyses = if early_release_enabled {
        drop(preliminary_module_analyses);
        Vec::new()
    } else {
        preliminary_module_analyses
    };
    if early_release_enabled {
        drop(std::mem::take(&mut module_export_tables));
        // A whole superseded generation (preliminary analyses + round-1 export
        // tables) was just dropped; return its pages before the final round
        // re-materializes every module's type surface on top of them.
        crate::metrics::release_free_memory();
    }
    // The per-file scope fallback (`module_scope_by_file`, consulted when a
    // declaration's pre-attached `resolution_scope` is incomplete) must be
    // available DURING the final module-analysis round, not just in the check
    // phase: signature collection resolves parameter types through local aliases
    // whose attached scope carries no import layers (`type BtnProps =
    // React.ComponentProps<…>`), and without the fallback the alias silently
    // degrades to `unknown` and the degraded signature is baked into the
    // module's value symbols and export table. The PRELIMINARY analysis round
    // deliberately runs without the map: its outputs are superseded by this
    // round, and resolving the full import graph twice measurably regresses
    // check time/memory on large cyclic programs (zod).
    ctx.begin_resolution_stage();
    let module_scope_map = module_scope_by_file_map(&parsed_files, &module_resolution_scopes, &ctx);
    ctx.set_module_scope_by_file(module_scope_map);
    let augmentation_insertions_before_final = augmentation_value_insertion_count();
    let type_collection_start = Instant::now();
    let final_analysis_parallel = analysis_worker_count > 1
        && std::env::var("SURGE_PARALLEL_ANALYSIS_FINAL").as_deref() != Ok("0");
    let module_analyses = if final_analysis_parallel {
        collect_module_analyses_with_bindings_parallel(
            &parsed_files,
            &local_type_declarations_by_module,
            &module_import_bindings,
            true,
            ctx,
            timings.as_ref(),
            analysis_worker_count,
        )
    } else {
        collect_module_analyses_with_bindings(
            parsed_files,
            &local_type_declarations_by_module,
            &module_import_bindings,
            true,
            ctx,
            timings.as_ref(),
        )
    };
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_analysis_collection += type_collection_start.elapsed()
    });
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_declaration_collection_start.elapsed()
    });
    record_rss_stage(
        timings.as_ref(),
        "final_module_analysis",
        program_start.elapsed(),
    );
    census_external.module_analysis_entries = module_analyses
        .iter()
        .filter(|analysis| analysis.is_some())
        .count() as u64;
    census_external.module_analysis_estimated_bytes =
        census_external.module_analysis_entries * std::mem::size_of::<ModuleAnalysis>() as u64;
    census_external.symbol_entries = global_symbols.iter().count() as u64;
    census_external.symbol_estimated_bytes = census_external.symbol_entries
        * (std::mem::size_of::<Arc<str>>()
            + std::mem::size_of::<crate::symbols::SymbolInfoHandle>()) as u64;
    emit_type_graph_census(
        "after_final_module_analysis",
        Some(&ctx),
        &store,
        *census_external,
    );
    report_eq_probe(
        &parsed_files,
        &preliminary_module_analyses,
        &module_analyses,
        &preliminary_module_import_bindings,
        &module_import_bindings,
        augmentation_insertions_before_final,
    );
    drop(preliminary_module_analyses);
    ModuleBinding {
        module_binding_start,
        module_export_tables,
        module_import_bindings,
        module_resolution_scopes,
        module_analyses,
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
    }
}

fn finalize_module_bindings(
    parsed_files: &mut Vec<ParsedProgramFile>,
    ctx: &mut CheckerContext,
    timings: &Option<Arc<Mutex<ProgramTimings>>>,
    program_start: Instant,
    binding: ModuleBinding,
    globals: GlobalCollection,
) -> ProgramCheckSharedState {
    let ModuleBinding {
        module_binding_start,
        module_export_tables: superseded_module_export_tables,
        mut module_import_bindings,
        mut module_resolution_scopes,
        module_analyses,
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
    } = binding;
    let GlobalCollection {
        global_symbols,
        script_values,
        function_signatures,
        global_type_declarations,
        ..
    } = globals;
    // The final analyses are built; the remaining pipeline reads declaration
    // files' statements only for their import/export binding surface
    // (`resolve_module_imports` matches `ImportDeclaration`,
    // `resolve_module_export_tables` matches specifier-bearing
    // `ExportDeclaration`s), and under `skipLibCheck` the check phase drops
    // them entirely. Shedding the declaration bodies here — before the binding
    // rounds and the check-phase peak — releases the bulk of the dependency
    // `.d.ts` ASTs several hundred megabytes earlier than the full release
    // below. The eq-stats probe re-reads full analyses, so it keeps them.
    if ctx.options.skip_lib_check && !eq_probe_enabled() {
        for parsed_file in parsed_files.iter_mut() {
            retain_declaration_binding_surface(parsed_file);
        }
        crate::metrics::release_free_memory();
    }
    let export_resolution_start = Instant::now();
    drop(superseded_module_export_tables);
    let mut module_export_tables = {
        let local_module_export_tables = module_analyses
            .iter()
            .map(|analysis| {
                analysis
                    .as_ref()
                    .map(|analysis| analysis.local_export_table.clone())
            })
            .collect::<Vec<_>>();
        ctx.begin_resolution_stage();
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, ctx)
    };
    record_program_timing(timings.as_ref(), |timings| {
        timings.final_export_table_resolution += export_resolution_start.elapsed()
    });
    refresh_reexported_namespace_objects(&parsed_files, &mut module_export_tables);
    let import_binding_start = Instant::now();
    drop(std::mem::take(&mut module_import_bindings));
    module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &module_analyses,
        &module_export_tables,
        &module_resolution_scopes,
        ctx,
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.import_binding_resolution += import_binding_start.elapsed()
    });
    let mut module_analyses = module_analyses;
    refine_imported_value_exports(
        parsed_files,
        &local_type_declarations_by_module,
        &preliminary_module_import_bindings,
        &mut module_analyses,
        &mut module_export_tables,
        &mut module_import_bindings,
        &module_resolution_scopes,
        ctx,
        timings.as_ref(),
    );
    let scope_build_start = Instant::now();
    drop(std::mem::take(&mut module_resolution_scopes));
    module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &module_import_bindings,
        timings.as_ref(),
    );
    drop(local_type_declarations_by_module);
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });
    let module_scope_map = module_scope_by_file_map(&parsed_files, &module_resolution_scopes, &ctx);
    ctx.set_module_scope_by_file(module_scope_map);
    ctx.jsx_intrinsic_elements_declarer =
        locate_jsx_intrinsic_elements_declarer(&parsed_files, &module_export_tables);
    // The resolved (re-export-expanded) export tables were only consumed by
    // import binding and the JSX locator; the check phase reads the analyses'
    // local export tables through `shared_state`.
    drop(module_export_tables);
    sync_global_this_symbol(ctx);
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_binding += module_binding_start.elapsed()
    });
    record_rss_stage(timings.as_ref(), "module_binding", program_start.elapsed());
    let script_type_declarations = {
        let mut table = clone_type_declaration_table(
            &global_type_declarations,
            timings.as_ref(),
            TableCloneKind::General,
        );
        // Declaration merging, not first-wins: a global interface split across a
        // script file and an ambient `declare global` block (`NodeJS.ProcessEnv`,
        // which several packages re-open) must carry every contributor's members,
        // not just whichever table was populated first.
        crate::symbols::merge_shared_table_into(
            &mut table,
            ctx.ambient_global_type_declarations.as_ref(),
        );
        table
    };
    let merged_module_import_bindings =
        merge_module_import_bindings(&module_import_bindings, &preliminary_module_import_bindings);
    drop(module_import_bindings);
    drop(preliminary_module_import_bindings);
    crate::metrics::release_free_memory();
    let shared_state = ProgramCheckSharedState {
        script_type_declarations,
        global_symbols,
        script_values,
        function_signatures,
        module_analyses,
        module_import_bindings: merged_module_import_bindings,
        module_resolution_scopes,
    };
    record_rss_stage(
        timings.as_ref(),
        "preliminary_release",
        program_start.elapsed(),
    );
    shared_state
}

// Per-file value tables for cross-module `typeof`. When a consumer resolves an
// imported type alias whose body contains `typeof <localValue>`, the value is
// declared in the alias's module, not the consumer's — so a per-file value
// table (consulted via `ctx.file_name`, which `with_file_name` sets to the
// declaring file during alias resolution) is needed. Built once here, before
// the (possibly parallel) check phase, so every job shares it read-only and the
// result is order-independent. The check loop is untouched. `module_analyses`'s
// `local_symbols` carries only function signatures, so a fresh
// `collect_exportable_value_symbols` pass is required to capture `const`/`class`
// value declarations. The seed table omits the ambient globals (they are added
// as a parent fallback inside the collector); the result is consulted via `get`
// only, so the parent fallback covers them.
/// Seed the module-local value tables before export publication instead of
/// only after the final analysis round. `SURGE_EARLY_MODULE_LOCAL_VALUES=0`
/// restores the old ordering.
///
/// A type published through a *value* export is resolved during
/// `bind_and_analyze_modules`, and a `typeof <value>` inside it reaches for the
/// declaring module's local value table — which that pass builds only after it
/// finishes. So the `typeof` member died at the sentinel while the rest of the
/// instantiation survived, which is what left tRPC's exported client open in
/// every consumer.
fn early_module_local_values_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SURGE_EARLY_MODULE_LOCAL_VALUES").as_deref() != Ok("0")
    })
}

pub(crate) fn build_module_local_values(
    parsed_files: &[ParsedProgramFile],
    module_analyses: &[Option<ModuleAnalysis>],
    module_import_bindings: &[Option<ModuleImportBindings>],
    ctx: &mut CheckerContext,
) {
    let saved_file_name = ctx.file_name.clone();
    let saved_type_declarations = std::mem::take(&mut ctx.type_declarations);
    let mut module_local_values: surge_ts_types::fx::FxHashMap<Arc<str>, Arc<SymbolTable>> =
        surge_ts_types::fx::FxHashMap::default();
    // Declaration modules are included: a library annotation chain routinely
    // crosses `typeof <importedValue>` (radix's
    // `ComponentPropsWithoutRef<typeof Primitive.button>`), which resolves
    // through this map under the declaring file's name.
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module {
            continue;
        }
        let Some(analysis) = module_analyses[file_index].as_ref() else {
            continue;
        };
        // The table is only ever consulted to resolve a `typeof <value>`
        // appearing in THIS file's own declarations/annotations (resolution
        // runs under the declaring file's name), so a file whose parse tree
        // contains no `typeof` type node can never be consulted and its
        // entry is skipped outright. This covers declaration files too:
        // their seed of Arc-shared handles is cheap to build, but the entry
        // is what pins each `.d.ts` module's symbol graph past the per-file
        // release after its check. Containment comes from the parse-time
        // `contains_typeof` source-text scan (a typeof type node can only
        // come from the keyword; an identifier substring only costs the
        // old eager build). `SURGE_LV_FILTER=0` restores unconditional
        // building; the `SURGE_LV_PROBE` accessor probe warns on any
        // consult miss.
        // A lazy body return also resolves its body against this table.
        let consulted_by_body_returns = !parsed_file.file_kind.is_declaration()
            && crate::checks::function::lazy_body_returns(&parsed_file.file_name);
        if local_values_typeof_filter_enabled()
            && !parsed_file.contains_typeof
            && !consulted_by_body_returns
        {
            continue;
        }
        let mut seed = SymbolTable::new();
        if let Some(bindings) = module_import_bindings[file_index].as_ref() {
            for (name, symbol) in bindings.symbols.iter_shared() {
                let _ = seed.insert_shared(name.clone(), symbol.clone());
            }
        }
        for (name, symbol) in analysis.local_symbols.iter_shared() {
            let _ = seed.insert_shared(name.clone(), symbol.clone());
        }
        if parsed_file.file_kind.is_declaration() {
            // A `typeof X` inside a declaration module targets either an
            // imported value (the binding symbol already carries its
            // export-table type) or an exported declaration (its typed
            // symbol sits in the local export table, computed during
            // binding). Reusing those Arc-shared handles covers both;
            // running the full exportable-value collection here instead
            // re-resolves every annotation of every dependency `.d.ts`
            // (unnamed: 27GB peak RSS, >6min).
            for (name, symbol) in analysis.local_export_table.symbols.iter_shared() {
                let _ = seed.insert_shared(name.clone(), symbol.clone());
            }
            module_local_values.insert(Arc::from(parsed_file.file_name.as_str()), Arc::new(seed));
            continue;
        }
        ctx.file_name = parsed_file.file_name.clone();
        ctx.type_declarations = analysis.local_type_declarations.as_ref().clone();
        let table = crate::modules::collect_exportable_value_symbols(
            &parsed_file.statements,
            &analysis.local_type_declarations,
            &seed,
            None,
            parsed_file.is_module,
            &ctx,
        );
        module_local_values.insert(Arc::from(parsed_file.file_name.as_str()), Arc::new(table));
    }
    ctx.file_name = saved_file_name;
    ctx.type_declarations = saved_type_declarations;
    ctx.set_module_local_values_by_file(module_local_values);
}

fn emit_check_phase_retention_census(
    label: &str,
    ctx: &CheckerContext,
    store: &Arc<ProgramTypeStore>,
    shared_state: &ProgramCheckSharedState,
    parsed_files: &[ParsedProgramFile],
) {
    if crate::metrics::retention_census_enabled() {
        let signature_refs = shared_state
            .function_signatures
            .values()
            .collect::<Vec<_>>();
        crate::metrics::emit_retention_census(
            label,
            Some(&ctx),
            &store,
            crate::metrics::RetentionCensusView {
                module_analyses: Some(&shared_state.module_analyses),
                module_import_bindings: Some(&shared_state.module_import_bindings),
                module_resolution_scopes: Some(&shared_state.module_resolution_scopes),
                parsed_files: Some(&parsed_files),
                global_symbols: Some(&shared_state.global_symbols),
                function_signatures: Some(&signature_refs),
                ..Default::default()
            },
        );
    }
}

// All cross-file program state now lives in `shared_state`; the per-file check
// phase receives only the current file plus `shared_state`, never the file
// slice. Under `skipLibCheck`, that phase skips declaration files outright, so
// their parse trees are dead from here on. Releasing them before the heaviest
// checking phase removes the dependency `.d.ts` / default-lib ASTs that
// dominate peak RSS on dependency-heavy projects. Without `skipLibCheck` the
// check phase still walks declaration statements, so they are kept.
fn release_declaration_asts(
    parsed_files: &mut Vec<ParsedProgramFile>,
    ctx: &CheckerContext,
    timings: &Option<Arc<Mutex<ProgramTimings>>>,
    program_start: Instant,
) {
    if ctx.options.skip_lib_check {
        for parsed_file in parsed_files.iter_mut() {
            if parsed_file.file_kind.is_declaration() {
                parsed_file.statements = Vec::new();
            }
        }
        crate::metrics::release_free_memory();
        record_rss_stage(
            timings.as_ref(),
            "declaration_ast_release",
            program_start.elapsed(),
        );
    }
}

fn run_check_phase(
    parsed_files: &mut Vec<ParsedProgramFile>,
    shared_state: &mut ProgramCheckSharedState,
    ctx: &mut CheckerContext,
    timings: &Option<Arc<Mutex<ProgramTimings>>>,
    program_start: Instant,
    jobs: usize,
) {
    let worker_count = resolve_worker_count(jobs, &parsed_files);
    crate::metrics::release_free_memory();
    set_check_phase(true);
    let file_results = if worker_count <= 1 {
        check_program_files_serial(parsed_files, shared_state, &ctx, timings.clone())
    } else {
        check_program_files_parallel(
            &parsed_files,
            &shared_state,
            &ctx,
            worker_count,
            timings.clone(),
        )
    };

    record_rss_stage(timings.as_ref(), "check_phase", program_start.elapsed());

    let mut deduper = DiagnosticDeduper::with_existing(&ctx.diagnostics);
    for result in file_results {
        deduper.extend(&mut ctx.diagnostics, result.diagnostics);
        ctx.stats.suppressed_diagnostics_total += result.stats.suppressed_diagnostics_total;
        ctx.stats.suppressed_declaration_diagnostics_total +=
            result.stats.suppressed_declaration_diagnostics_total;
        ctx.stats.suppressed_rust_only_diagnostics_total +=
            result.stats.suppressed_rust_only_diagnostics_total;
    }
}

fn finish_program_run(
    ctx: CheckerContext,
    store: &Arc<ProgramTypeStore>,
    timings: &Option<Arc<Mutex<ProgramTimings>>>,
    timings_enabled: bool,
    program_start: Instant,
    census_external: CensusExternalRetention,
    skip_teardown: bool,
) -> ProgramCheckResult {
    if timings.is_some() {
        let cache_stats = ctx.program_cache_stats();
        record_program_timing(timings.as_ref(), |timings| {
            timings.cache_stats = Some(cache_stats)
        });
    }
    let declaration_environment_stats = ctx.declaration_environment_store.stats();
    let substitution_store_stats = ctx.substitution_store.stats();
    emit_type_graph_census("before_cache_cleanup", Some(&ctx), &store, census_external);
    set_check_phase(false);
    if !skip_teardown {
        ctx.clear_program_type_caches();
        store.clear();
        // The run-scoped thread-local caches are otherwise cleared only at the
        // START of the next run, so in a one-shot process they survive to exit —
        // the namespace-alias tables in particular retain whole per-module
        // declaration tables.
        crate::paths::clear_canonicalize_cache();
        crate::modules::clear_relative_module_cache();
        crate::modules::clear_star_export_unresolved_cache();
        crate::modules::clear_namespace_alias_table_cache();
        crate::modules::set_node_esm_files(None);
        crate::modules::set_allow_arbitrary_extensions(false);
        crate::metrics::release_free_memory();
    }
    emit_type_graph_census("after_cache_cleanup", Some(&ctx), &store, census_external);
    emit_type_graph_census("before_process_exit", Some(&ctx), &store, census_external);
    crate::context::report_local_values_consults(ctx.module_local_values_by_file.len());
    let (diagnostics, stats) = ctx.finish_with_stats();
    record_rss_stage(timings.as_ref(), "finish", program_start.elapsed());
    render_dts_expansion_summary();

    if let Some(timings) = timings.as_ref() {
        render_program_rss_stages(timings);
        if timings_enabled {
            render_program_type_store_stats(
                &store,
                declaration_environment_stats,
                substitution_store_stats,
            );
            render_program_timings(timings);
        }
    }

    if std::env::var_os("SURGE_ALLOCATION_CENSUS").is_some() {
        eprintln!("ParsedType clone census:");
        let census = surge_ts_syntax::clone_census::parsed_type_clone_census();
        let total: u64 = census.iter().map(|(_, count)| count).sum();
        for (name, count) in census {
            eprintln!("  {name}: {count}");
        }
        eprintln!("  total: {total}");
    }

    ProgramCheckResult {
        diagnostics,
        stats,
        syntax_errors: false,
    }
}

fn inject_generated_default_lib_inputs(files: &mut Vec<SourceFileInput>, no_lib: bool) {
    if no_lib
        || files.iter().any(|file| {
            crate::default_lib::is_generated_default_lib_file_name(&file.file_name)
                || crate::default_lib::is_physical_default_lib_file_name(&file.file_name)
        })
    {
        return;
    }

    let mut default_lib_inputs = load_generated_default_lib_inputs(false, None);
    if default_lib_inputs.is_empty() {
        return;
    }

    default_lib_inputs.extend(files.drain(..));
    *files = default_lib_inputs;
}

fn local_values_typeof_filter_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_LV_FILTER").as_deref() != Ok("0"))
}

fn census_check_milestone(completed: usize, total: usize) -> Option<&'static str> {
    if !type_graph_census_enabled() || total == 0 {
        return None;
    }
    let quarter = total.div_ceil(4);
    let half = total.div_ceil(2);
    let three_quarters = (total * 3).div_ceil(4);
    if completed == quarter {
        Some("after_checking_25_percent")
    } else if completed == half {
        Some("after_checking_50_percent")
    } else if completed == three_quarters {
        Some("after_checking_75_percent")
    } else {
        None
    }
}

/// Finds the declaration-file module whose export table carries the JSX
/// intrinsic-elements interface (`JSX.IntrinsicElements`, the key an
/// `import * as React` would re-qualify as `React.JSX.IntrinsicElements`).
/// Only dependency/root declaration files are considered so a user module
/// re-declaring the name cannot hijack the program-wide fallback.
/// Rebuilds a module-namespace object that one module re-exports from another
/// (`import * as inner from "./inner"; export { inner }`) against the *final*
/// export tables.
///
/// The object an importer binds is materialized at import-binding time, and the
/// bindings the final analysis round runs under were built from the
/// preliminary analyses — where `SURGE_THIN_PRELIM` degrades every variable to
/// `unknown`. A module that only *uses* the import re-resolves later, but one
/// that re-exports it bakes that thin object into its own export table, and
/// nothing refreshes it: `typeof recast.types.builders` (tRPC's jscodeshift
/// surface, through `ast-types`) stayed open for the rest of the run.
///
/// Only the top level is rebuilt, from the tag `tag_namespace_type_with_module_path`
/// leaves on the object, so a module namespace that (transitively) contains
/// itself cannot recurse.
fn refresh_reexported_namespace_objects(
    parsed_files: &[ParsedProgramFile],
    module_export_tables: &mut [Option<ModuleExportTable>],
) {
    let mut index_by_module_path: surge_ts_types::fx::FxHashMap<&str, usize> =
        surge_ts_types::fx::FxHashMap::default();
    for (index, parsed_file) in parsed_files.iter().enumerate() {
        index_by_module_path.insert(
            crate::modules::imports::strip_typescript_extension(&parsed_file.file_name),
            index,
        );
    }

    let mut replacements: Vec<(usize, Arc<str>, surge_ts_types::Type)> = Vec::new();
    for (index, export_table) in module_export_tables.iter().enumerate() {
        let Some(export_table) = export_table else {
            continue;
        };
        for (name, symbol) in export_table.symbols.iter_shared() {
            let Some(module_path) = namespace_object_module_path(&symbol.ty) else {
                continue;
            };
            let Some(&source_index) = index_by_module_path.get(module_path) else {
                continue;
            };
            if source_index == index {
                continue;
            }
            let Some(source_table) = module_export_tables[source_index].as_ref() else {
                continue;
            };
            let refreshed = crate::modules::imports::tag_namespace_type_with_module_path(
                crate::modules::namespace_export_object_type(source_table),
                &parsed_files[source_index].file_name,
            );
            if refreshed != symbol.ty {
                replacements.push((index, name.clone(), refreshed));
            }
        }
    }

    for (index, name, refreshed) in replacements {
        let Some(export_table) = module_export_tables[index].as_mut() else {
            continue;
        };
        let Some(symbol) = export_table.symbols.get(name.as_ref()) else {
            continue;
        };
        let mut symbol = symbol.clone();
        symbol.ty = refreshed;
        export_table
            .symbols
            .insert_shared(name.to_string(), Arc::new(symbol));
        export_table.namespace_export_object_type = None;
    }
}

/// The module path a namespace-import object was tagged with, if the type is
/// one (`typeof import("<path>")`).
fn namespace_object_module_path(ty: &surge_ts_types::Type) -> Option<&str> {
    let surge_ts_types::Type::Object(object) = ty else {
        return None;
    };
    object
        .alias_name
        .as_deref()?
        .strip_prefix("typeof import(\"")?
        .strip_suffix("\")")
}

fn locate_jsx_intrinsic_elements_declarer(
    parsed_files: &[ParsedProgramFile],
    module_export_tables: &[Option<crate::modules::ModuleExportTable>],
) -> Option<(Arc<TypeDeclarationTable>, String)> {
    const CANDIDATE_KEYS: [&str; 2] = ["JSX.IntrinsicElements", "React.JSX.IntrinsicElements"];

    for key in CANDIDATE_KEYS {
        for (parsed_file, table) in parsed_files.iter().zip(module_export_tables) {
            if !parsed_file.file_kind.is_declaration() {
                continue;
            }
            let Some(table) = table else { continue };
            if table.type_declarations.get(key).is_some() {
                return Some((table.type_declarations.clone(), key.to_string()));
            }
        }
    }
    None
}

fn clone_type_declaration_table(
    table: &TypeDeclarationTable,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    kind: TableCloneKind,
) -> TypeDeclarationTable {
    record_type_declaration_table_clone(timings, table.len(), kind);
    table.clone()
}

static FAST_PROCESS_EXIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The caller exits the process as soon as the result is rendered, so the
/// end-of-run teardown — dropping the parsed program and clearing every
/// program-lifetime cache — is work nothing will observe. Library callers that
/// keep the process alive must leave this off: the teardown is what bounds a
/// second run's memory. Ignored while any RSS, timing, or census
/// instrumentation is on, since those report the teardown itself.
pub fn set_fast_process_exit(enabled: bool) {
    FAST_PROCESS_EXIT.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

fn fast_process_exit() -> bool {
    FAST_PROCESS_EXIT.load(std::sync::atomic::Ordering::Relaxed)
}
