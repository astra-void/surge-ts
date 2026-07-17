use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExportDeclaration, ParsedStatement, ParserWorker};
use surge_ts_types::{FunctionType, ProgramTypeStore, with_program_type_store};

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.
pub(crate) use crate::metrics::*;

use crate::context::{CheckerContext, CheckerOptions, CompatibilityStats, FileKind};
use crate::default_lib::load_generated_default_lib_inputs;
use crate::driver::validate_direct_utility_aliases;
use crate::driver::{sync_global_this_symbol, validate_local_type_declarations};
use crate::modules::{ModuleExportTable, ModuleImportBindings, resolve_module_export_tables};
use crate::paths::canonicalize_if_exists_string;
use crate::symbols::{
    SymbolTable, TypeDeclarationScope, TypeDeclarationTable, clone_symbol_info_handle,
};

mod ambient;
mod binding;
mod classes;
mod globals;
mod schedule;
mod statements;
mod unused_locals;

pub(crate) use ambient::*;
pub(crate) use binding::*;
pub(crate) use classes::*;
pub(crate) use globals::*;
pub(crate) use statements::*;

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
    pub(crate) statements: Vec<ParsedStatement>,
    pub(crate) parser_errors: Vec<String>,
    pub(crate) is_module: bool,
    pub(crate) file_kind: FileKind,
    /// Module-wide identifier reads (see [`surge_ts_syntax::ParsedSource`]),
    /// retained only when `noUnusedLocals` is enabled; empty otherwise.
    pub(crate) module_reads: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProgramCheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub stats: CompatibilityStats,
}

#[derive(Debug, Clone)]
struct ProgramCheckSharedState {
    global_type_declarations: TypeDeclarationTable,
    /// Prebuilt global+ambient declaration table for script (non-module) files.
    /// Built once on the main thread before the check-phase fan-out: inserting
    /// allocates into the shared global arena, whose bump allocator is not
    /// thread-safe, so workers must only clone this table (an index copy), never
    /// rebuild it.
    script_type_declarations: TypeDeclarationTable,
    global_symbols: SymbolTable,
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
    local_export_table: ModuleExportTable,
}

impl ModuleAnalysis {
    pub(crate) fn local_type_declarations(&self) -> &Arc<TypeDeclarationTable> {
        &self.local_type_declarations
    }

    pub(crate) fn local_symbols(&self) -> &SymbolTable {
        &self.local_symbols
    }

    pub(crate) fn local_export_table(&self) -> &ModuleExportTable {
        &self.local_export_table
    }
}

// --- Temporary SURGE_EQ_STATS probe: measures how many files' preliminary and
// final module analyses are already output-equal (the ceiling for any
// equality-based final-round skip) and how much preliminary time they carry.
// Remove once the skip decision is recorded.

pub(crate) fn eq_probe_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_EQ_STATS").is_some())
}

/// Per-round, per-file counter samples taken around one module's analysis.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EqProbeVisit {
    pub(crate) elapsed: std::time::Duration,
    pub(crate) signature_scope_consults: u64,
    pub(crate) degraded_resolutions: u64,
    pub(crate) augmentation_insertions_after: u64,
}

static EQ_PROBE_VISITS: std::sync::OnceLock<Mutex<HashMap<usize, Vec<EqProbeVisit>>>> =
    std::sync::OnceLock::new();

pub(crate) fn record_eq_probe_visit(file_index: usize, visit: EqProbeVisit) {
    let store = EQ_PROBE_VISITS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut visits) = store.lock() {
        visits.entry(file_index).or_default().push(visit);
    }
}

static SCOPE_FALLBACK_CONSULTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn record_scope_fallback_consult() {
    SCOPE_FALLBACK_CONSULTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn scope_fallback_consult_count() -> u64 {
    SCOPE_FALLBACK_CONSULTS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Degraded (`had_error`, uninternable) named-type resolutions. A file whose
/// preliminary analysis observed one may resolve differently in the final round
/// (a clean interned entry can exist by then), so the equality predictor must
/// exclude it.
static DEGRADED_RESOLUTIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn record_degraded_resolution() {
    DEGRADED_RESOLUTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn degraded_resolution_count() -> u64 {
    DEGRADED_RESOLUTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// First-wins `declare global` value insertions (`lower_global_augmentation_values`).
/// They mutate `ctx.ambient_global_symbols` *during* the analysis loop, so a file
/// analyzed before the inserting file saw fewer globals in the preliminary round
/// than it will in the final round.
static AUGMENTATION_VALUE_INSERTIONS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub(crate) fn record_augmentation_value_insertion() {
    AUGMENTATION_VALUE_INSERTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn augmentation_value_insertion_count() -> u64 {
    AUGMENTATION_VALUE_INSERTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Opt-in per-module analysis-time dump (`SURGE_MODULE_TIME_DUMP=<path>`): one
/// `round\tmicros\tfile_name` line per analyzed module, so the real per-module
/// analysis cost distribution can be joined with the import-edge dump for
/// SCC critical-path weighting. Off by default; zero-cost when unset.
fn module_time_sink() -> Option<&'static Mutex<std::fs::File>> {
    static SINK: std::sync::OnceLock<Option<Mutex<std::fs::File>>> = std::sync::OnceLock::new();
    SINK.get_or_init(|| {
        let path = std::env::var_os("SURGE_MODULE_TIME_DUMP")?;
        std::fs::File::create(path).ok().map(Mutex::new)
    })
    .as_ref()
}

pub(crate) fn module_time_dump_enabled() -> bool {
    module_time_sink().is_some()
}

pub(crate) fn record_module_time(round: u64, file_name: &str, micros: u128) {
    use std::io::Write;
    if let Some(sink) = module_time_sink()
        && let Ok(mut file) = sink.lock()
    {
        let _ = writeln!(file, "{round}\t{micros}\t{file_name}");
    }
}

fn eq_probe_verbose() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_EQ_STATS_VERBOSE").is_some())
}

fn eq_probe_symbol_tables_equal(a: &SymbolTable, b: &SymbolTable) -> bool {
    let count_a = a.iter_shared().count();
    if count_a != b.iter_shared().count() {
        return false;
    }
    a.iter_shared().all(|(name, symbol_a)| {
        b.get_shared(name)
            .is_some_and(|symbol_b| symbol_a.kind == symbol_b.kind && symbol_a.ty == symbol_b.ty)
    })
}

fn eq_probe_explain_symbol_tables(label: &str, a: &SymbolTable, b: &SymbolTable) {
    let count_a = a.iter_shared().count();
    let count_b = b.iter_shared().count();
    if count_a != count_b {
        eprintln!("[eq-stats]   {label}: entry count {count_a} vs {count_b}");
        let names_a: HashSet<&str> = a.iter_shared().map(|(n, _)| n.as_ref()).collect();
        let names_b: HashSet<&str> = b.iter_shared().map(|(n, _)| n.as_ref()).collect();
        for only_a in names_a.difference(&names_b) {
            eprintln!("[eq-stats]     only-prelim: {only_a}");
        }
        for only_b in names_b.difference(&names_a) {
            eprintln!("[eq-stats]     only-final: {only_b}");
        }
        return;
    }
    for (name, symbol_a) in a.iter_shared() {
        match b.get_shared(name) {
            None => eprintln!("[eq-stats]   {label}: '{name}' missing in final"),
            Some(symbol_b) => {
                if symbol_a.kind != symbol_b.kind {
                    eprintln!(
                        "[eq-stats]   {label}: '{name}' kind {:?} vs {:?}",
                        symbol_a.kind, symbol_b.kind
                    );
                } else if symbol_a.ty != symbol_b.ty {
                    eprintln!(
                        "[eq-stats]   {label}: '{name}' type '{}' vs '{}'",
                        symbol_a.ty.name(),
                        symbol_b.ty.name()
                    );
                }
            }
        }
    }
}

fn eq_probe_explain_analyses(file_name: &str, a: &ModuleAnalysis, b: &ModuleAnalysis) {
    eprintln!("[eq-stats]  explain {file_name}:");
    eq_probe_explain_symbol_tables("local_symbols", &a.local_symbols, &b.local_symbols);
    eq_probe_explain_symbol_tables(
        "export_symbols",
        &a.local_export_table.symbols,
        &b.local_export_table.symbols,
    );
    let table_a = &a.local_export_table;
    let table_b = &b.local_export_table;
    match (&table_a.default_symbol, &table_b.default_symbol) {
        (Some(sa), Some(sb)) if sa.ty != sb.ty => eprintln!(
            "[eq-stats]   default_symbol: '{}' vs '{}'",
            sa.ty.name(),
            sb.ty.name()
        ),
        (Some(_), None) | (None, Some(_)) => eprintln!("[eq-stats]   default_symbol presence"),
        _ => {}
    }
    match (
        &table_a.export_assignment_symbol,
        &table_b.export_assignment_symbol,
    ) {
        (Some(sa), Some(sb)) if sa.ty != sb.ty => eprintln!(
            "[eq-stats]   export_assignment: '{}' vs '{}'",
            sa.ty.name(),
            sb.ty.name()
        ),
        (Some(_), None) | (None, Some(_)) => eprintln!("[eq-stats]   export_assignment presence"),
        _ => {}
    }
    if table_a.type_declarations.len() != table_b.type_declarations.len() {
        eprintln!(
            "[eq-stats]   export type_declarations: {} vs {}",
            table_a.type_declarations.len(),
            table_b.type_declarations.len()
        );
    }
}

fn eq_probe_analyses_equal(a: &ModuleAnalysis, b: &ModuleAnalysis) -> bool {
    let table_a = &a.local_export_table;
    let table_b = &b.local_export_table;
    eq_probe_symbol_tables_equal(&a.local_symbols, &b.local_symbols)
        && eq_probe_symbol_tables_equal(&table_a.symbols, &table_b.symbols)
        && match (&table_a.default_symbol, &table_b.default_symbol) {
            (None, None) => true,
            (Some(sa), Some(sb)) => sa.ty == sb.ty,
            _ => false,
        }
        && match (
            &table_a.export_assignment_symbol,
            &table_b.export_assignment_symbol,
        ) {
            (None, None) => true,
            (Some(sa), Some(sb)) => sa.ty == sb.ty,
            _ => false,
        }
        && table_a.type_declarations.len() == table_b.type_declarations.len()
}

fn eq_probe_scopes_equal(
    a: &Option<Arc<TypeDeclarationScope>>,
    b: &Option<Arc<TypeDeclarationScope>>,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            Arc::ptr_eq(a, b) || {
                let (la, lb) = (a.layers(), b.layers());
                la.len() == lb.len() && la.iter().zip(lb).all(|(x, y)| Arc::ptr_eq(x, y))
            }
        }
        _ => false,
    }
}

fn eq_probe_declarations_equal(
    a: &crate::symbols::TypeDeclarationInfo,
    b: &crate::symbols::TypeDeclarationInfo,
) -> bool {
    use crate::symbols::TypeDeclarationInfo;
    match (a, b) {
        (TypeDeclarationInfo::Alias(a), TypeDeclarationInfo::Alias(b)) => {
            a.name == b.name
                && Arc::ptr_eq(&a.body, &b.body)
                && eq_probe_scopes_equal(&a.resolution_scope, &b.resolution_scope)
        }
        (TypeDeclarationInfo::Interface(a), TypeDeclarationInfo::Interface(b)) => {
            a.name == b.name
                && Arc::ptr_eq(&a.body, &b.body)
                && eq_probe_scopes_equal(&a.resolution_scope, &b.resolution_scope)
        }
        _ => false,
    }
}

fn eq_probe_declaration_tables_equal(a: &TypeDeclarationTable, b: &TypeDeclarationTable) -> bool {
    a.len() == b.len()
        && a.iter().all(|(name, da)| {
            b.get(name.as_ref())
                .is_some_and(|db| eq_probe_declarations_equal(da, db))
        })
}

fn eq_probe_layer_equal(a: &Arc<TypeDeclarationTable>, b: &Arc<TypeDeclarationTable>) -> bool {
    Arc::ptr_eq(a, b) || eq_probe_declaration_tables_equal(a, b)
}

fn eq_probe_bindings_equal(a: &ModuleImportBindings, b: &ModuleImportBindings) -> bool {
    eq_probe_symbol_tables_equal(&a.symbols, &b.symbols)
        && eq_probe_layer_equal(&a.type_declarations, &b.type_declarations)
        && a.namespace_alias_layers.len() == b.namespace_alias_layers.len()
        && a.namespace_alias_layers
            .iter()
            .zip(&b.namespace_alias_layers)
            .all(|(x, y)| eq_probe_layer_equal(x, y))
}

fn report_eq_probe(
    parsed_files: &[ParsedProgramFile],
    preliminary: &[Option<ModuleAnalysis>],
    final_analyses: &[Option<ModuleAnalysis>],
    preliminary_bindings: &[Option<ModuleImportBindings>],
    final_bindings: &[Option<ModuleImportBindings>],
    augmentation_insertions_before_final: u64,
) {
    if !eq_probe_enabled() {
        return;
    }
    let visits_by_file = EQ_PROBE_VISITS
        .get()
        .and_then(|store| store.lock().ok().map(|d| d.clone()))
        .unwrap_or_default();
    let mut analyzed = 0usize;
    let mut equal = 0usize;
    let mut predicted = 0usize;
    let mut unsound = 0usize;
    let mut excluded_consults = 0usize;
    let mut excluded_degraded = 0usize;
    let mut excluded_augmentation = 0usize;
    let mut excluded_bindings = 0usize;
    let mut prelim_total = std::time::Duration::ZERO;
    let mut prelim_equal = std::time::Duration::ZERO;
    let mut final_total = std::time::Duration::ZERO;
    let mut final_equal = std::time::Duration::ZERO;
    let mut final_predicted = std::time::Duration::ZERO;
    for (index, (p, f)) in preliminary.iter().zip(final_analyses.iter()).enumerate() {
        let (Some(p), Some(f)) = (p, f) else { continue };
        analyzed += 1;
        let visits = visits_by_file.get(&index);
        let prelim_visit = visits.and_then(|v| v.first().copied()).unwrap_or_default();
        let final_visit = visits.and_then(|v| v.get(1).copied()).unwrap_or_default();
        let prelim_time = prelim_visit.elapsed;
        let final_time = final_visit.elapsed;
        prelim_total += prelim_time;
        final_total += final_time;
        let bindings_equal = match (&preliminary_bindings[index], &final_bindings[index]) {
            (Some(a), Some(b)) => eq_probe_bindings_equal(a, b),
            (None, None) => true,
            _ => false,
        };
        let output_equal = eq_probe_analyses_equal(p, f);
        if !bindings_equal {
            excluded_bindings += 1;
        } else if prelim_visit.signature_scope_consults != 0 {
            excluded_consults += 1;
        } else if prelim_visit.degraded_resolutions != 0 {
            excluded_degraded += 1;
        } else if prelim_visit.augmentation_insertions_after != augmentation_insertions_before_final
        {
            excluded_augmentation += 1;
        }
        let is_predicted = bindings_equal
            && prelim_visit.signature_scope_consults == 0
            && prelim_visit.degraded_resolutions == 0
            && prelim_visit.augmentation_insertions_after == augmentation_insertions_before_final;
        if output_equal {
            equal += 1;
            prelim_equal += prelim_time;
            final_equal += final_time;
        }
        if is_predicted {
            predicted += 1;
            final_predicted += final_time;
            if !output_equal {
                unsound += 1;
                eprintln!(
                    "[eq-stats] UNSOUND predicted-equal but output differs: {}",
                    parsed_files[index].file_name
                );
                if eq_probe_verbose() {
                    eq_probe_explain_analyses(&parsed_files[index].file_name, p, f);
                }
            }
        }
    }
    eprintln!(
        "[eq-stats] excluded: bindings={excluded_bindings} consults={excluded_consults} \
         degraded={excluded_degraded} augmentation={excluded_augmentation}"
    );
    eprintln!(
        "[eq-stats] analyzed={analyzed} output_equal={equal} ({:.1}%) \
         prelim_time_equal={:.2}s/{:.2}s ({:.1}%) final_time_equal={:.2}s/{:.2}s ({:.1}%)",
        100.0 * equal as f64 / analyzed.max(1) as f64,
        prelim_equal.as_secs_f64(),
        prelim_total.as_secs_f64(),
        100.0 * prelim_equal.as_secs_f64() / prelim_total.as_secs_f64().max(f64::EPSILON),
        final_equal.as_secs_f64(),
        final_total.as_secs_f64(),
        100.0 * final_equal.as_secs_f64() / final_total.as_secs_f64().max(f64::EPSILON),
    );
    eprintln!(
        "[eq-stats] predicted_skip={predicted} ({:.1}%) unsound={unsound} \
         final_time_predicted={:.2}s/{:.2}s ({:.1}%)",
        100.0 * predicted as f64 / analyzed.max(1) as f64,
        final_predicted.as_secs_f64(),
        final_total.as_secs_f64(),
        100.0 * final_predicted.as_secs_f64() / final_total.as_secs_f64().max(f64::EPSILON),
    );
}

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
    let store = ProgramTypeStore::new();
    with_program_type_store(store.clone(), || {
        check_program_with_stats_and_jobs_inner(files, options, jobs, store)
    })
}

fn check_program_with_stats_and_jobs_inner(
    files: Vec<SourceFileInput>,
    options: CheckerOptions,
    jobs: usize,
    store: Arc<ProgramTypeStore>,
) -> ProgramCheckResult {
    if files.is_empty() {
        return ProgramCheckResult {
            diagnostics: Vec::new(),
            stats: CompatibilityStats::default(),
        };
    }

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

    let parse_start = Instant::now();
    let mut parsed_files = parse_program_files(files, jobs, timings.as_ref());
    let ast_nodes = parsed_files
        .iter()
        .map(|file| file.statements.len() as u64)
        .sum::<u64>();
    let mut census_external = CensusExternalRetention {
        ast_nodes,
        ast_estimated_bytes: ast_nodes * std::mem::size_of::<ParsedStatement>() as u64,
        source_text_bytes,
        ..CensusExternalRetention::default()
    };
    record_program_timing(timings.as_ref(), |timings| {
        timings.parsing += parse_start.elapsed()
    });
    record_rss_stage(timings.as_ref(), "parsing", program_start.elapsed());
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
    ctx.timings = timings.clone();
    ctx.set_module_file_index_by_identity(module_file_index_by_identity);
    emit_type_graph_census("after_loading_parsing", Some(&ctx), &store, census_external);

    let mut global_symbols = SymbolTable::new();
    let mut function_signatures = HashMap::new();

    let ambient_collection_start = Instant::now();
    emit_parser_diagnostics(&parsed_files, &mut ctx);
    collect_ambient_globals(&parsed_files, &mut ctx, timings.as_ref());
    crate::driver::collect_global_augmentations(&parsed_files, &mut ctx);
    collect_ambient_modules(&parsed_files, &mut ctx, timings.as_ref());
    record_program_timing(timings.as_ref(), |timings| {
        timings.ambient_collection += ambient_collection_start.elapsed()
    });
    record_rss_stage(
        timings.as_ref(),
        "ambient_collection",
        program_start.elapsed(),
    );

    let type_declaration_collection_start = Instant::now();
    collect_global_type_declarations(&parsed_files, &mut ctx, timings.as_ref());
    record_program_timing(timings.as_ref(), |timings| {
        timings.root_source_global_collection += type_declaration_collection_start.elapsed()
    });
    let global_type_declarations = clone_type_declaration_table(
        &ctx.type_declarations,
        timings.as_ref(),
        TableCloneKind::General,
    );
    collect_global_function_signatures(
        &parsed_files,
        &mut global_symbols,
        &mut function_signatures,
        &mut ctx,
    );
    collect_global_variables(&parsed_files, &mut global_symbols, &mut ctx);
    record_rss_stage(
        timings.as_ref(),
        "global_collection",
        program_start.elapsed(),
    );

    // PRELIMINARY PASS: collect types and resolve imports/exports to make them available for function signature collection
    let type_collection_start = Instant::now();
    let (
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
        preliminary_type_diagnostics,
    ) = collect_preliminary_module_type_bindings(&parsed_files, &mut ctx, timings.as_ref());
    for diagnostic in preliminary_type_diagnostics {
        ctx.push(diagnostic);
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
    // per-worker contexts, and arena ownership transfer are in place.
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
            &mut ctx,
            timings.as_ref(),
            analysis_worker_count,
        )
    } else {
        collect_module_analyses_with_bindings(
            &parsed_files,
            &local_type_declarations_by_module,
            &preliminary_module_import_bindings,
            false,
            &mut ctx,
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
        census_external,
    );
    crate::metrics::emit_retention_census(
        "after_preliminary_analysis",
        Some(&ctx),
        &store,
        crate::metrics::RetentionCensusView {
            preliminary_module_analyses: Some(&preliminary_module_analyses),
            preliminary_module_import_bindings: Some(&preliminary_module_import_bindings),
            global_symbols: Some(&global_symbols),
            ..Default::default()
        },
    );

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
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, &mut ctx)
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
        &mut ctx,
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
        &mut ctx,
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
    ctx.set_module_scope_by_file(module_scope_by_file_map(
        &parsed_files,
        &module_resolution_scopes,
    ));
    let augmentation_insertions_before_final = augmentation_value_insertion_count();
    let type_collection_start = Instant::now();
    let module_analyses = collect_module_analyses_with_bindings(
        &parsed_files,
        &local_type_declarations_by_module,
        &module_import_bindings,
        true,
        &mut ctx,
        timings.as_ref(),
    );
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
        census_external,
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
            if !parsed_file.file_kind.is_declaration() {
                continue;
            }
            parsed_file.statements.retain(|statement| match statement {
                ParsedStatement::ImportDeclaration(_) => true,
                ParsedStatement::ExportDeclaration(export) => !matches!(
                    export.as_ref(),
                    ParsedExportDeclaration::Statement { .. }
                        | ParsedExportDeclaration::Default { .. }
                ),
                _ => false,
            });
            parsed_file.statements.shrink_to_fit();
        }
        crate::metrics::release_free_memory();
    }
    let export_resolution_start = Instant::now();
    module_export_tables = {
        let local_module_export_tables = module_analyses
            .iter()
            .map(|analysis| {
                analysis
                    .as_ref()
                    .map(|analysis| analysis.local_export_table.clone())
            })
            .collect::<Vec<_>>();
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, &mut ctx)
    };
    record_program_timing(timings.as_ref(), |timings| {
        timings.final_export_table_resolution += export_resolution_start.elapsed()
    });
    let import_binding_start = Instant::now();
    drop(std::mem::take(&mut module_import_bindings));
    module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &module_analyses,
        &module_export_tables,
        &module_resolution_scopes,
        &mut ctx,
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
    drop(local_type_declarations_by_module);
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });
    ctx.set_module_scope_by_file(module_scope_by_file_map(
        &parsed_files,
        &module_resolution_scopes,
    ));
    ctx.jsx_intrinsic_elements_declarer =
        locate_jsx_intrinsic_elements_declarer(&parsed_files, &module_export_tables);
    // The resolved (re-export-expanded) export tables were only consumed by
    // import binding and the JSX locator; the check phase reads the analyses'
    // local export tables through `shared_state`.
    drop(module_export_tables);
    sync_global_this_symbol(&mut ctx);
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
        for (name, declaration) in ctx.ambient_global_type_declarations.iter() {
            let _ = table.insert(name.clone(), declaration.clone());
        }
        table
    };
    let merged_module_import_bindings =
        merge_module_import_bindings(&module_import_bindings, &preliminary_module_import_bindings);
    drop(module_import_bindings);
    drop(preliminary_module_import_bindings);
    crate::metrics::release_free_memory();
    let mut shared_state = ProgramCheckSharedState {
        global_type_declarations,
        script_type_declarations,
        global_symbols,
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
    {
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
            let Some(analysis) = shared_state.module_analyses[file_index].as_ref() else {
                continue;
            };
            let mut seed = SymbolTable::new();
            if let Some(bindings) = shared_state.module_import_bindings[file_index].as_ref() {
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
                module_local_values
                    .insert(Arc::from(parsed_file.file_name.as_str()), Arc::new(seed));
                continue;
            }
            ctx.file_name = parsed_file.file_name.clone();
            ctx.type_declarations = analysis.local_type_declarations.as_ref().clone();
            let table = crate::modules::collect_exportable_value_symbols(
                &parsed_file.statements,
                &analysis.local_type_declarations,
                &seed,
                None,
                &ctx,
            );
            module_local_values.insert(Arc::from(parsed_file.file_name.as_str()), Arc::new(table));
        }
        ctx.file_name = saved_file_name;
        ctx.type_declarations = saved_type_declarations;
        ctx.set_module_local_values_by_file(module_local_values);
    }
    record_rss_stage(
        timings.as_ref(),
        "module_local_values",
        program_start.elapsed(),
    );
    if crate::metrics::retention_census_enabled() {
        let signature_refs = shared_state
            .function_signatures
            .values()
            .collect::<Vec<_>>();
        crate::metrics::emit_retention_census(
            "before_check_phase",
            Some(&ctx),
            &store,
            crate::metrics::RetentionCensusView {
                module_analyses: Some(&shared_state.module_analyses),
                module_import_bindings: Some(&shared_state.module_import_bindings),
                module_resolution_scopes: Some(&shared_state.module_resolution_scopes),
                global_symbols: Some(&shared_state.global_symbols),
                function_signatures: Some(&signature_refs),
                ..Default::default()
            },
        );
    }

    // All cross-file program state now lives in `shared_state`; the per-file check
    // phase receives only the current file plus `shared_state`, never the file
    // slice. Under `skipLibCheck`, that phase skips declaration files outright, so
    // their parse trees are dead from here on. Releasing them before the heaviest
    // checking phase removes the dependency `.d.ts` / default-lib ASTs that
    // dominate peak RSS on dependency-heavy projects. Without `skipLibCheck` the
    // check phase still walks declaration statements, so they are kept.
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

    let worker_count = resolve_worker_count(jobs, &parsed_files);
    crate::metrics::release_free_memory();
    let file_results = if worker_count <= 1 {
        check_program_files_serial(&mut parsed_files, &mut shared_state, &ctx, timings.clone())
    } else {
        // From here on, workers only read arena-backed tables. Freezing makes a
        // late allocation — a data race on the non-thread-safe bump allocator —
        // fail loudly instead of corrupting memory. Serial checking is exempt:
        // single-threaded allocation is sound.
        freeze_worker_reachable_arenas(&shared_state, &ctx);
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
    if crate::metrics::retention_census_enabled() {
        let signature_refs = shared_state
            .function_signatures
            .values()
            .collect::<Vec<_>>();
        crate::metrics::emit_retention_census(
            "after_check_phase",
            Some(&ctx),
            &store,
            crate::metrics::RetentionCensusView {
                module_analyses: Some(&shared_state.module_analyses),
                module_import_bindings: Some(&shared_state.module_import_bindings),
                module_resolution_scopes: Some(&shared_state.module_resolution_scopes),
                global_symbols: Some(&shared_state.global_symbols),
                function_signatures: Some(&signature_refs),
                ..Default::default()
            },
        );
    }
    // Checking is complete and the diagnostics are extracted: the cross-file
    // program state and every remaining parse tree are dead. Dropping them here
    // (rather than at function exit, after the finish measurements) makes the
    // finish footprint reflect what a long-lived host would actually retain.
    drop(shared_state);
    drop(parsed_files);

    if timings.is_some() {
        let cache_stats = ctx.program_cache_stats();
        record_program_timing(timings.as_ref(), |timings| {
            timings.cache_stats = Some(cache_stats)
        });
    }
    let declaration_environment_stats = ctx.declaration_environment_store.stats();
    let substitution_store_stats = ctx.substitution_store.stats();
    emit_type_graph_census("before_cache_cleanup", Some(&ctx), &store, census_external);
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
    crate::metrics::release_free_memory();
    emit_type_graph_census("after_cache_cleanup", Some(&ctx), &store, census_external);
    emit_type_graph_census("before_process_exit", Some(&ctx), &store, census_external);
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

    ProgramCheckResult { diagnostics, stats }
}

/// Minimum source bytes assigned to a worker before `AUTO_JOBS` parallelizes the
/// parse phase. Parse cost tracks byte length, and files are pulled through a
/// shared cursor so large declaration/default-lib files don't stall one worker.
/// This is a calibration threshold — tune with `bench:test`.
const MIN_BYTES_PER_PARSE_WORKER: usize = 256 * 1024;

fn resolve_parse_worker_count(jobs: usize, files: &[SourceFileInput]) -> usize {
    let file_count = files.len();
    if file_count <= 1 {
        return 1;
    }

    let requested = if jobs == AUTO_JOBS {
        let cores = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let bytes: usize = files.iter().map(|file| file.source_text.len()).sum();
        let by_work = (bytes / MIN_BYTES_PER_PARSE_WORKER).max(1);
        cores.min(by_work)
    } else {
        jobs
    };

    requested.max(1).min(file_count)
}

fn parse_program_files(
    files: Vec<SourceFileInput>,
    jobs: usize,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> Vec<ParsedProgramFile> {
    let worker_count = resolve_parse_worker_count(jobs, &files);
    if worker_count <= 1 {
        let mut parser = ParserWorker::new();
        return files
            .iter()
            .map(|input| parse_program_file(&mut parser, input, timings))
            .collect();
    }

    let next_index = AtomicUsize::new(0);
    let timings_owned = timings.cloned();

    let mut indexed = thread::scope(|scope| {
        let next_index = &next_index;
        let files = &files;
        let mut handles = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let timings = timings_owned.clone();
            handles.push(scope.spawn(move || {
                // One arena per parse thread; never shared across threads.
                let mut parser = ParserWorker::new();
                let mut worker_results = Vec::new();
                loop {
                    let file_index = next_index.fetch_add(1, Ordering::Relaxed);
                    if file_index >= files.len() {
                        break;
                    }
                    worker_results.push((
                        file_index,
                        parse_program_file(&mut parser, &files[file_index], timings.as_ref()),
                    ));
                }
                worker_results
            }));
        }

        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("parse worker panicked"))
            .collect::<Vec<_>>()
    });

    indexed.sort_by_key(|(file_index, _)| *file_index);
    indexed.into_iter().map(|(_, parsed)| parsed).collect()
}

fn parse_program_file(
    parser: &mut ParserWorker,
    input: &SourceFileInput,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> ParsedProgramFile {
    record_program_counter(|c| c.files_total += 1);
    if classify_file_kind(&input.file_name) == FileKind::GeneratedDeclaration {
        record_program_counter(|c| c.generated_default_lib_files += 1);
    }

    let parse_start = Instant::now();
    let parsed = parser.parse(&input.source_text, &input.file_name);
    let parse_duration = parse_start.elapsed();
    let file_name = parsed.file_name;
    let file_kind = classify_file_kind(&file_name);
    record_program_timing(timings, |timings| match file_kind {
        FileKind::DependencyDeclaration => {
            timings.dependency_declaration_parse_time += parse_duration
        }
        FileKind::GeneratedDeclaration | FileKind::PhysicalDefaultLib => {
            timings.generated_default_lib_parse_time += parse_duration
        }
        FileKind::RootSource | FileKind::RootDeclaration => {}
    });
    record_program_counter(|c| match file_kind {
        FileKind::RootSource => c.root_source_files += 1,
        FileKind::RootDeclaration | FileKind::DependencyDeclaration => {
            if matches!(file_kind, FileKind::DependencyDeclaration) {
                c.dependency_declaration_files += 1;
            }
            if matches!(file_kind, FileKind::RootDeclaration) {
                c.root_source_files += 1;
            }
            if matches!(file_kind, FileKind::DependencyDeclaration) {
                c.parsed_dependency_declaration_files += 1;
            } else {
                c.parsed_root_source_files += 1;
            }
        }
        FileKind::GeneratedDeclaration => {}
        FileKind::PhysicalDefaultLib => {
            c.generated_default_lib_files += 1;
        }
    });
    ParsedProgramFile {
        file_name: file_name.clone(),
        has_export_default: input.source_text.contains("export default"),
        statements: parsed.statements,
        parser_errors: parsed.parser_errors,
        is_module: parsed.is_module,
        file_kind: classify_file_kind(&file_name),
        module_reads: parsed.module_reads,
    }
}

fn classify_file_kind(file_name: &str) -> FileKind {
    if is_declaration_file_name(file_name) {
        if is_generated_declaration_file_name(file_name) {
            return FileKind::GeneratedDeclaration;
        }

        // Physical default libs live under `.../typescript/lib/lib.*.d.ts`.
        // Classify them ahead of the generic dependency-declaration check so
        // they route through the real ambient-lowering pipeline rather than
        // being skipped like ordinary `node_modules` declarations.
        if crate::default_lib::is_physical_default_lib_file_name(file_name) {
            return FileKind::PhysicalDefaultLib;
        }

        // Package exports, `typesVersions`, type conditions, `@types`, and
        // re-export chains all converge on a physical declaration path before
        // this classification. Only installed-package declarations get the
        // aggressive declaration-backed policy; path-mapped declarations and
        // project-reference outputs outside dependency roots stay
        // `RootDeclaration` and retain user-authored checking semantics.
        let normalized = file_name.replace('\\', "/");
        if normalized.contains("/node_modules/") {
            return FileKind::DependencyDeclaration;
        }

        return FileKind::RootDeclaration;
    }

    FileKind::RootSource
}

fn is_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}

fn is_generated_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.contains("/.nuxt/")
        || lower.contains("/.generated/")
        || lower.contains("/generated-libs/")
        || lower.contains("/generated/")
        || lower.ends_with(".generated.d.ts")
        || lower.ends_with(".generated.d.mts")
        || lower.ends_with(".generated.d.cts")
}

#[cfg(test)]
mod file_kind_tests {
    use super::{FileKind, classify_file_kind};

    #[test]
    fn dependency_declaration_extensions_are_classified_lazily() {
        for file_name in [
            "/repo/node_modules/pkg/index.d.ts",
            "/repo/node_modules/pkg/index.d.mts",
            "/repo/node_modules/pkg/index.d.cts",
            r"C:\repo\node_modules\@types\pkg\index.d.ts",
            "/repo/node_modules/.pnpm/pkg@1/node_modules/pkg/index.d.ts",
        ] {
            assert_eq!(
                classify_file_kind(file_name),
                FileKind::DependencyDeclaration,
                "{file_name}"
            );
        }
    }

    #[test]
    fn user_and_generated_declarations_keep_distinct_policies() {
        assert_eq!(
            classify_file_kind("/repo/types/path-mapped.d.ts"),
            FileKind::RootDeclaration
        );
        assert_eq!(
            classify_file_kind("/repo/project-reference/dist/index.d.ts"),
            FileKind::RootDeclaration
        );
        assert_eq!(
            classify_file_kind("/repo/.generated/router.d.ts"),
            FileKind::GeneratedDeclaration
        );
        assert_eq!(
            classify_file_kind("/repo/node_modules/pkg/index.ts"),
            FileKind::RootSource
        );
    }
}

fn emit_parser_diagnostics(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());

        for message in &parsed_file.parser_errors {
            ctx.push(Diagnostic::surge_parser_error(
                message.clone(),
                parsed_file.file_name.clone(),
            ));
        }
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

fn check_program_files_serial(
    parsed_files: &mut [ParsedProgramFile],
    shared_state: &mut ProgramCheckSharedState,
    ctx: &CheckerContext,
    timings: Option<Arc<Mutex<ProgramTimings>>>,
) -> Vec<FileCheckResult> {
    let mut results = Vec::with_capacity(parsed_files.len());

    // One reused context for the whole pass, exactly like a single parallel
    // worker: `check_program_file` already isolates per-file state (see
    // `begin_file_check`), and serial/parallel diagnostic equality is an
    // asserted invariant. Cloning the (large) context per file was a measured
    // ~3% of check-phase time on tRPC.
    let mut local_ctx = ctx.clone();
    local_ctx.diagnostics.clear();
    local_ctx.stats = CompatibilityStats::default();

    // SURGE_CHECK_CACHE_ISOLATION=1: restore the program-wide resolution caches
    // to their pre-file state after every file, so each file observes exactly
    // the analysis-end cache contents. Measures whether any diagnostic depends
    // on cache entries seeded by earlier files' checking (the blocker for
    // deterministic parallel checking). Experiment probe — not a production
    // mode.
    let cache_isolation = std::env::var_os("SURGE_CHECK_CACHE_ISOLATION").is_some();

    let file_count = parsed_files.len();
    for file_index in 0..file_count {
        let cache_snapshot = (cache_isolation && !parsed_files[file_index].statements.is_empty())
            .then(|| {
                (
                    local_ctx
                        .program_resolved_generic_types
                        .lock()
                        .ok()
                        .map(|m| m.clone()),
                    local_ctx
                        .program_instantiations
                        .lock()
                        .ok()
                        .map(|m| m.clone()),
                    local_ctx
                        .physical_interface_instantiations
                        .lock()
                        .ok()
                        .map(|m| m.clone()),
                    local_ctx
                        .physical_interface_declaration_templates
                        .lock()
                        .ok()
                        .map(|m| m.clone()),
                    local_ctx
                        .physical_interface_method_instantiations
                        .lock()
                        .ok()
                        .map(|m| m.clone()),
                    local_ctx
                        .physical_interface_overload_instantiations
                        .lock()
                        .ok()
                        .map(|m| m.clone()),
                )
            });
        let result = check_program_file(
            file_index,
            &parsed_files[file_index],
            shared_state,
            &mut local_ctx,
            timings.as_ref(),
        );
        if let Some(snapshot) = cache_snapshot {
            if let (Some(saved), Ok(mut live)) =
                (snapshot.0, local_ctx.program_resolved_generic_types.lock())
            {
                *live = saved;
            }
            if let (Some(saved), Ok(mut live)) =
                (snapshot.1, local_ctx.program_instantiations.lock())
            {
                *live = saved;
            }
            if let (Some(saved), Ok(mut live)) = (
                snapshot.2,
                local_ctx.physical_interface_instantiations.lock(),
            ) {
                *live = saved;
            }
            if let (Some(saved), Ok(mut live)) = (
                snapshot.3,
                local_ctx.physical_interface_declaration_templates.lock(),
            ) {
                *live = saved;
            }
            if let (Some(saved), Ok(mut live)) = (
                snapshot.4,
                local_ctx.physical_interface_method_instantiations.lock(),
            ) {
                *live = saved;
            }
            if let (Some(saved), Ok(mut live)) = (
                snapshot.5,
                local_ctx.physical_interface_overload_instantiations.lock(),
            ) {
                *live = saved;
            }
        }
        results.push(result);
        // The file is fully checked, and per-file program state is only ever
        // read under the file's own index (checking never consults another
        // file's parse tree, analysis, or bindings — cross-file resolution
        // goes through the scope/value maps on the context). Everything
        // index-scoped can therefore free before the next file's checking
        // allocates. Cross-file `Arc`-shared pieces (scope layers, exported
        // symbol handles held by importers' bindings) survive through their
        // remaining owners; only the genuinely-dead remainder frees.
        parsed_files[file_index].statements = Vec::new();
        shared_state.module_analyses[file_index] = None;
        shared_state.module_import_bindings[file_index] = None;
        // Per-file inference churn leaves freed-but-dirty pages that otherwise
        // accumulate against the footprint across the whole phase.
        if (file_index + 1) % 256 == 0 {
            crate::metrics::release_free_memory();
        }
        if let Some(label) = census_check_milestone(file_index + 1, file_count) {
            emit_type_graph_census(
                label,
                Some(&local_ctx),
                &local_ctx.program_type_store,
                CensusExternalRetention::default(),
            );
        }
    }

    results
}

/// Sentinel passed as `jobs` to request automatic worker-count selection.
const AUTO_JOBS: usize = 0;

/// Minimum parsed top-level statements assigned to a worker before `AUTO_JOBS`
/// spins up another thread. Per-worker `CheckerContext` clones and thread spawn
/// are not free, so tiny programs stay serial. Statement count is measured after
/// the `skipLibCheck` trim above, so cleared declaration files correctly count as
/// near-zero work. This is a calibration threshold — tune with `bench:test`.
const MIN_STATEMENTS_PER_WORKER: usize = 500;

fn resolve_worker_count(jobs: usize, parsed_files: &[ParsedProgramFile]) -> usize {
    let file_count = parsed_files.len();
    if file_count <= 1 {
        return 1;
    }

    // Checking is coupled to the shared resolution caches in an order-visible
    // way (whether a file hits an entry seeded by an earlier-checked file can
    // flip a rendered display form), so naive concurrency changes bytes. The
    // parallel path is serial-equivalent regardless: workers speculate against
    // a frozen cache snapshot and a single-threaded commit publishes their
    // insertions in serial file order, re-checking conflicted files (see
    // `crate::speculative`). `--jobs auto` therefore sizes from available
    // cores, gated by parsed work so tiny programs stay serial; an explicit
    // `--jobs N` is an upper bound on the same byte-identical path.
    let requested = if jobs == AUTO_JOBS {
        let total_statements: usize = parsed_files.iter().map(|file| file.statements.len()).sum();
        let by_work = total_statements / MIN_STATEMENTS_PER_WORKER;
        let cores = thread::available_parallelism()
            .map(|cores| cores.get())
            .unwrap_or(1);
        cores.min(by_work)
    } else {
        jobs
    };

    requested.max(1).min(file_count)
}

/// Freeze every arena directly reachable by check-phase workers (through
/// `shared_state` or the cloned worker contexts) so that any allocation after
/// the fan-out panics deterministically. Clones of a table share the arena, so
/// freezing one handle freezes every clone. See [`CheckerArena::freeze`].
fn freeze_worker_reachable_arenas(shared_state: &ProgramCheckSharedState, ctx: &CheckerContext) {
    shared_state
        .global_type_declarations
        .arena_handle()
        .freeze();
    shared_state
        .script_type_declarations
        .arena_handle()
        .freeze();
    ctx.type_declarations.arena_handle().freeze();
    ctx.ambient_global_type_declarations.arena_handle().freeze();
    for analysis in shared_state.module_analyses.iter().flatten() {
        analysis.local_type_declarations.arena_handle().freeze();
        analysis
            .local_export_table
            .type_declarations
            .arena_handle()
            .freeze();
    }
    for bindings in shared_state.module_import_bindings.iter().flatten() {
        for layer in bindings.scope_layers() {
            layer.arena_handle().freeze();
        }
    }
    for scope in shared_state.module_resolution_scopes.iter().flatten() {
        for layer in scope.layers() {
            layer.arena_handle().freeze();
        }
    }
}

fn check_program_files_parallel(
    parsed_files: &[ParsedProgramFile],
    shared_state: &ProgramCheckSharedState,
    ctx: &CheckerContext,
    worker_count: usize,
    timings: Option<Arc<Mutex<ProgramTimings>>>,
) -> Vec<FileCheckResult> {
    debug_assert!(worker_count > 1, "serial checking uses the dedicated path");

    // Serial-equivalent speculative checking: workers never write the six
    // order-visible program caches. Each speculates against an immutable
    // snapshot taken here plus a private overlay, recording per file which
    // cache keys it observed missing; the single-threaded commit pass below
    // publishes insertions in serial file order and re-checks any file whose
    // hit/miss pattern a serial run would not have produced, making the final
    // diagnostics and cache contents byte-identical to `--jobs 1`. See
    // `crate::speculative` for the model and its induction argument.
    let live = crate::speculative::LiveCacheHandles::capture(ctx);
    let base = Arc::new(crate::speculative::CacheSnapshots::capture(&live));
    let stc_phase_start = Instant::now();

    let next_index = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);

    let results = thread::scope(|scope| {
        let next_index = &next_index;
        let completed = &completed;
        let mut handles = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let shared_state = shared_state;
            let timings = timings.clone();
            let mut local_ctx = ctx.clone();
            local_ctx.diagnostics.clear();
            local_ctx.stats = CompatibilityStats::default();
            let session = Arc::new(crate::speculative::CheckSession::new(
                live.clone(),
                base.clone(),
            ));

            handles.push(scope.spawn(move || {
                let type_store = local_ctx.program_type_store.clone();
                let worker_results = with_program_type_store(type_store, || {
                    crate::speculative::with_check_session(session.clone(), || {
                        let mut worker_results = Vec::new();
                        loop {
                            // `fetch_add` hands each worker an ascending file
                            // sequence, so a worker's overlay only ever holds
                            // entries from files earlier in serial order than
                            // the one it is checking.
                            let file_index = next_index.fetch_add(1, Ordering::Relaxed);
                            if file_index >= parsed_files.len() {
                                break;
                            }
                            session.begin_file(file_index);
                            worker_results.push(check_program_file(
                                file_index,
                                &parsed_files[file_index],
                                shared_state,
                                &mut local_ctx,
                                timings.as_ref(),
                            ));
                            let completed_count = completed.fetch_add(1, Ordering::Relaxed) + 1;
                            if let Some(label) =
                                census_check_milestone(completed_count, parsed_files.len())
                            {
                                emit_type_graph_census(
                                    label,
                                    Some(&local_ctx),
                                    &local_ctx.program_type_store,
                                    CensusExternalRetention::default(),
                                );
                            }
                        }
                        worker_results
                    })
                });
                (worker_results, session.take_file_logs())
            }));
        }

        let mut outputs = Vec::with_capacity(handles.len());
        for handle in handles {
            outputs.push(
                handle
                    .join()
                    .expect("parallel project checking worker panicked"),
            );
        }
        outputs
    });

    let worker_phase = stc_phase_start.elapsed();
    let commit_phase_start = Instant::now();
    let mut slots: Vec<Option<FileCheckResult>> = (0..parsed_files.len()).map(|_| None).collect();
    let mut logs = Vec::with_capacity(parsed_files.len());
    for (worker_results, worker_logs) in results {
        for result in worker_results {
            let file_index = result.file_index;
            slots[file_index] = Some(result);
        }
        logs.extend(worker_logs);
    }
    logs.sort_by_key(|log| log.file_index);

    // Deterministic commit: file order, independent of worker completion order.
    let cap = crate::infer::types::cache::generic_instantiation_bucket_cap();
    let mut published = surge_ts_types::fx::FxHashSet::default();
    let mut dirty = surge_ts_types::fx::FxHashSet::default();
    let mut stats = crate::speculative::StcCommitStats::default();
    let mut recheck_ctx: Option<CheckerContext> = None;
    for log in &logs {
        match crate::speculative::commit_file_log(
            &live,
            log,
            &mut published,
            &dirty,
            cap,
            &mut stats,
        ) {
            crate::speculative::CommitVerdict::Clean => {}
            crate::speculative::CommitVerdict::MissConflict
            | crate::speculative::CommitVerdict::DependencyConflict => {
                let file_index = log.file_index;
                dirty.insert(file_index);
                let local_ctx = recheck_ctx.get_or_insert_with(|| {
                    let mut local_ctx = ctx.clone();
                    local_ctx.diagnostics.clear();
                    local_ctx.stats = CompatibilityStats::default();
                    local_ctx
                });
                // Re-check against the now-committed cache state; by induction
                // this is exactly what a serial run would have observed at this
                // position, and the recheck's own insertions cannot conflict
                // (its base view already contains everything published). The
                // session reads the live maps directly — the commit pass is
                // single-threaded, so no snapshot clone is needed.
                let session = Arc::new(crate::speculative::CheckSession::new_live_reading(
                    live.clone(),
                ));
                let result = crate::speculative::with_check_session(session.clone(), || {
                    session.begin_file(file_index);
                    check_program_file(
                        file_index,
                        &parsed_files[file_index],
                        shared_state,
                        local_ctx,
                        timings.as_ref(),
                    )
                });
                slots[file_index] = Some(result);
                for recheck_log in session.take_file_logs() {
                    let complete = crate::speculative::apply_file_log(
                        &live,
                        &recheck_log,
                        &mut published,
                        cap,
                        &mut stats,
                    );
                    debug_assert!(complete, "recheck publication must be complete");
                }
            }
        }
    }
    if std::env::var_os("SURGE_STC_STATS").is_some() {
        let total_misses: usize = logs
            .iter()
            .map(crate::speculative::FileCacheLog::miss_count)
            .sum();
        eprintln!(
            "[stc] files={} clean={} miss_conflicts={} dep_conflicts={} published={} \
             skipped_existing={} cap_blocked={} total_misses={} worker_phase={:.2}s \
             commit_phase={:.2}s",
            stats.files,
            stats.clean_commits,
            stats.miss_conflicts,
            stats.dependency_conflicts,
            stats.published_entries,
            stats.merge_skipped_existing,
            stats.merge_cap_blocked,
            total_misses,
            worker_phase.as_secs_f64(),
            commit_phase_start.elapsed().as_secs_f64(),
        );
    }

    slots.into_iter().flatten().collect()
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

type DiagnosticDedupKey = (
    String,
    String,
    String,
    Option<surge_ts_diagnostics::TextSpan>,
);

fn diagnostic_dedup_key(diagnostic: &surge_ts_diagnostics::Diagnostic) -> DiagnosticDedupKey {
    (
        diagnostic.code.to_string(),
        diagnostic.file_name.clone(),
        diagnostic.message.clone(),
        diagnostic.span,
    )
}

/// Order-preserving diagnostic dedup backed by a key set. The previous
/// implementation rescanned the whole accumulated `Vec` (and re-rendered every
/// code to a `String`) for each incoming diagnostic, so merging N files' results
/// was O(D^2) in the total diagnostic count. Hoisting the set across the merge
/// keeps it O(D).
#[derive(Default)]
struct DiagnosticDeduper {
    seen: HashSet<DiagnosticDedupKey>,
}

impl DiagnosticDeduper {
    fn with_existing(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Self {
        Self {
            seen: diagnostics.iter().map(diagnostic_dedup_key).collect(),
        }
    }

    fn extend(
        &mut self,
        diagnostics: &mut Vec<surge_ts_diagnostics::Diagnostic>,
        new_diagnostics: Vec<surge_ts_diagnostics::Diagnostic>,
    ) {
        for diagnostic in new_diagnostics {
            if self.seen.insert(diagnostic_dedup_key(&diagnostic)) {
                diagnostics.push(diagnostic);
            }
        }
    }
}

fn extend_diagnostics_dedup(
    diagnostics: &mut Vec<surge_ts_diagnostics::Diagnostic>,
    new_diagnostics: Vec<surge_ts_diagnostics::Diagnostic>,
) {
    DiagnosticDeduper::with_existing(diagnostics).extend(diagnostics, new_diagnostics);
}

/// Finds the declaration-file module whose export table carries the JSX
/// intrinsic-elements interface (`JSX.IntrinsicElements`, the key an
/// `import * as React` would re-qualify as `React.JSX.IntrinsicElements`).
/// Only dependency/root declaration files are considered so a user module
/// re-declaring the name cannot hijack the program-wide fallback.
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

fn module_scope_by_file_map(
    parsed_files: &[ParsedProgramFile],
    module_resolution_scopes: &[Option<Arc<crate::symbols::TypeDeclarationScope>>],
) -> surge_ts_types::fx::FxHashMap<Arc<str>, Arc<crate::symbols::TypeDeclarationScope>> {
    parsed_files
        .iter()
        .zip(module_resolution_scopes.iter())
        .filter_map(|(parsed_file, scope)| {
            scope
                .as_ref()
                .map(|scope| (Arc::from(parsed_file.file_name.as_str()), scope.clone()))
        })
        .collect()
}

fn check_program_file(
    file_index: usize,
    parsed_file: &ParsedProgramFile,
    shared_state: &ProgramCheckSharedState,
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> FileCheckResult {
    ctx.begin_file_check(parsed_file.file_name.clone());

    if ctx.options.skip_lib_check && parsed_file.file_kind.is_declaration() {
        return FileCheckResult {
            file_index,
            diagnostics: Vec::new(),
            stats: CompatibilityStats::default(),
        };
    }

    if parsed_file.file_kind.is_declaration() {
        emit_unsupported_declaration_diagnostics(&parsed_file.statements, ctx);
        let diagnostics = std::mem::take(&mut ctx.diagnostics);
        let stats = std::mem::take(&mut ctx.stats);
        return FileCheckResult {
            file_index,
            diagnostics,
            stats,
        };
    }

    if parsed_file.is_module {
        let Some(module_analysis) = shared_state.module_analyses[file_index].as_ref() else {
            return FileCheckResult {
                file_index,
                diagnostics: Vec::new(),
                stats: CompatibilityStats::default(),
            };
        };

        let imported_bindings = shared_state.module_import_bindings[file_index].as_ref();

        let module_resolution_scope = shared_state.module_resolution_scopes[file_index]
            .as_ref()
            .cloned()
            .unwrap_or_else(|| {
                let mut layers = vec![module_analysis.local_type_declarations.clone()];
                if let Some(imported_bindings) = imported_bindings {
                    layers.extend(imported_bindings.scope_layers());
                }
                record_module_scope_cache_miss();
                Arc::new(TypeDeclarationScope::new(layers))
            });
        if shared_state.module_resolution_scopes[file_index].is_some() {
            record_module_scope_cache_hit();
        }

        let mut merged_symbols = ctx
            .ambient_global_symbols
            .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
        if let Some(imported_bindings) = imported_bindings {
            for (name, symbol) in imported_bindings.symbols.iter_shared() {
                let _ = merged_symbols.insert_shared(name.clone(), symbol.clone());
            }
        }
        for (name, symbol) in module_analysis.local_symbols.iter_shared() {
            let _ = merged_symbols.insert_shared(name.clone(), symbol.clone());
        }

        ctx.type_declarations = module_analysis.local_type_declarations.as_ref().clone();
        ctx.type_declaration_scope = Some(module_resolution_scope);
        ctx.set_symbols(
            merged_symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext),
        );

        let current_type_declarations = ctx.type_declarations.clone();
        let current_symbols = ctx
            .symbols
            .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
        let validation_symbols = crate::modules::collect_exportable_value_symbols(
            &parsed_file.statements,
            &current_type_declarations,
            &current_symbols,
            None,
            ctx,
        );
        let saved_symbols = std::mem::replace(&mut ctx.symbols, validation_symbols);

        let validation_start = Instant::now();
        validate_local_type_declarations(&parsed_file.statements, &parsed_file.file_name, ctx);
        let validation_duration = validation_start.elapsed();
        record_program_timing(timings, |timings| {
            timings.declaration_validation += validation_duration
        });
        record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
            metrics.validate_local_type_declarations_passes += 1;
            metrics.validate_local_type_declarations_items +=
                count_local_type_declarations_in_statements(&parsed_file.statements) as u64;
            metrics.validate_local_type_declarations_duration += validation_duration;
        });

        let utility_validation_start = Instant::now();
        validate_direct_utility_aliases(&parsed_file.statements, ctx);
        record_program_timing(timings, |timings| {
            timings.utility_alias_validation += utility_validation_start.elapsed()
        });

        let validation_symbols = std::mem::replace(&mut ctx.symbols, saved_symbols);

        let mut signature_ctx = ctx.clone();
        signature_ctx.diagnostics.clear();
        signature_ctx.reset_utility_diagnostic_keys();
        signature_ctx.resolved_named_types =
            std::sync::Arc::new(std::sync::Mutex::new(Default::default()));
        // Seed from `validation_symbols` rather than `merged_symbols`: it carries
        // the local `const`/`let`/`var` value symbols inferred during declaration
        // validation, so `typeof <localConst>` resolves inside parameter type
        // annotations (function signatures see them, not just type aliases).
        //
        // Only the file's own top-level function declarations are skipped (they
        // are re-registered below, and re-seeding them would trip the duplicate
        // signature check). Imported and global functions are kept so that
        // `typeof <importedFn>` in a parameter annotation still resolves.
        let mut signature_local_symbols = crate::symbols::SymbolTable::new();
        for (name, symbol) in validation_symbols.iter_shared() {
            let is_local_function_declaration =
                matches!(symbol.kind, crate::symbols::SymbolKind::Function)
                    && module_analysis.local_symbols.get(name).is_some();
            if !is_local_function_declaration {
                signature_local_symbols.insert_shared(name.clone(), symbol.clone());
            }
        }
        let mut final_function_signatures = HashMap::new();
        collect_function_signatures_from_statements(
            &parsed_file.statements,
            file_index,
            &mut signature_local_symbols,
            &mut final_function_signatures,
            &mut signature_ctx,
        );
        extend_diagnostics_dedup(&mut ctx.diagnostics, signature_ctx.diagnostics);

        ctx.module_value_fallback = Some(std::sync::Arc::new(validation_symbols));

        let statement_check_start = Instant::now();
        check_program_file_statements(
            &parsed_file.statements,
            file_index,
            &final_function_signatures,
            ctx,
        );
        ctx.module_value_fallback = None;

        if ctx.options.no_unused_locals && ctx.current_file_kind == FileKind::RootSource {
            unused_locals::emit_unused_module_bindings(
                &parsed_file.statements,
                &parsed_file.module_reads,
                ctx,
            );
        }
        record_program_timing(timings, |timings| {
            timings.per_file_statement_checking += statement_check_start.elapsed()
        });
    } else {
        // Clone the prebuilt global+ambient table (index copy only). Inserting
        // here would allocate into the shared global arena from a worker thread,
        // which the arena's freeze assertion forbids.
        ctx.type_declarations = clone_type_declaration_table(
            &shared_state.script_type_declarations,
            timings,
            TableCloneKind::General,
        );
        ctx.type_declaration_scope = None;

        let mut script_sym = shared_state
            .global_symbols
            .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
        for (name, symbol) in ctx.ambient_global_symbols.iter_handles() {
            let _ = script_sym.insert_handle(name.clone(), clone_symbol_info_handle(symbol));
        }
        ctx.set_symbols(script_sym);

        let current_type_declarations = ctx.type_declarations.clone();
        let current_symbols = ctx
            .symbols
            .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
        let validation_symbols = crate::modules::collect_exportable_value_symbols(
            &parsed_file.statements,
            &current_type_declarations,
            &current_symbols,
            None,
            ctx,
        );
        let saved_symbols = std::mem::replace(&mut ctx.symbols, validation_symbols);

        let validation_start = Instant::now();
        validate_local_type_declarations(&parsed_file.statements, &parsed_file.file_name, ctx);
        let validation_duration = validation_start.elapsed();
        record_program_timing(timings, |timings| {
            timings.declaration_validation += validation_duration
        });
        record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
            metrics.validate_local_type_declarations_passes += 1;
            metrics.validate_local_type_declarations_items +=
                count_local_type_declarations_in_statements(&parsed_file.statements) as u64;
            metrics.validate_local_type_declarations_duration += validation_duration;
        });

        let utility_validation_start = Instant::now();
        validate_direct_utility_aliases(&parsed_file.statements, ctx);
        record_program_timing(timings, |timings| {
            timings.utility_alias_validation += utility_validation_start.elapsed()
        });

        let validation_symbols = std::mem::replace(&mut ctx.symbols, saved_symbols);

        ctx.module_value_fallback = Some(std::sync::Arc::new(validation_symbols));

        let statement_check_start = Instant::now();
        check_program_file_statements(
            &parsed_file.statements,
            file_index,
            &shared_state.function_signatures,
            ctx,
        );
        ctx.module_value_fallback = None;
        record_program_timing(timings, |timings| {
            timings.per_file_statement_checking += statement_check_start.elapsed()
        });
    }

    let diagnostics = std::mem::take(&mut ctx.diagnostics);
    let stats = std::mem::take(&mut ctx.stats);

    FileCheckResult {
        file_index,
        diagnostics,
        stats,
    }
}

fn clone_type_declaration_table(
    table: &TypeDeclarationTable,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    kind: TableCloneKind,
) -> TypeDeclarationTable {
    record_type_declaration_table_clone(timings, table.len(), kind);
    table.clone()
}
