use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedFunctionDeclaration,
    ParsedImportKind, ParsedStatement, TextSpan, parse_source,
};
use typescript_rust_types::{
    FunctionType, snapshot_function_type_counters, snapshot_union_type_counters,
};

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::{CheckerContext, CheckerOptions, CompatibilityStats, FileKind};
use crate::default_lib::inject_generated_default_lib_snapshot_for_file_name;
use crate::driver::validate_direct_utility_aliases;
use crate::driver::{
    collect_global_augmentations_from_statements, collect_type_declarations,
    sync_global_this_symbol, validate_local_type_declarations,
};
use crate::load_default_lib_inputs;
use crate::modules::{
    ModuleExportTable, ModuleImportBindings, build_module_export_table,
    resolve_module_export_tables, resolve_module_imports,
};
use crate::paths::canonicalize_if_exists_string;
use crate::symbols::{
    SymbolTable, TypeDeclarationScope, TypeDeclarationTable, clone_symbol_info_handle,
};

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
    #[allow(dead_code)]
    pub(crate) source_text: String,
    pub(crate) statements: Vec<ParsedStatement>,
    pub(crate) parser_errors: Vec<String>,
    pub(crate) is_module: bool,
    pub(crate) file_kind: FileKind,
}

#[derive(Debug, Clone)]
pub struct ProgramCheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub stats: CompatibilityStats,
}

#[derive(Debug, Clone)]
struct ProgramCheckSharedState {
    global_type_declarations: TypeDeclarationTable,
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
struct ModuleAnalysis {
    local_type_declarations: Arc<TypeDeclarationTable>,
    local_symbols: SymbolTable,
    #[allow(dead_code)]
    local_function_signatures: HashMap<FunctionDeclarationLocation, FunctionType>,
    local_export_table: ModuleExportTable,
}

#[derive(Debug, Clone)]
struct AmbientModuleEntry {
    module_specifier: String,
    file: ParsedProgramFile,
    raw_export_table: ModuleExportTable,
}

#[derive(Debug, Default, Clone)]
struct ProgramCounters {
    files_total: u64,
    root_source_files: u64,
    dependency_declaration_files: u64,
    generated_default_lib_files: u64,
    parsed_root_source_files: u64,
    parsed_dependency_declaration_files: u64,
    parsed_generated_default_lib_files: u64,
    checker_arena_alloc_count: u64,
    arena_declaration_key_alloc_count: u64,
    arena_type_declaration_payload_alloc_count: u64,
    arena_object_type_payload_alloc_count: u64,
    type_declaration_payload_deep_clone_count: u64,
    object_type_payload_deep_clone_count: u64,
    object_type_alloc_count: u64,
    union_type_alloc_count: u64,
    function_type_alloc_count: u64,
    module_analysis_total_calls: u64,
    module_analysis_unique_files: u64,
    module_analysis_duplicate_calls: u64,
    type_declaration_table_clone_count: u64,
    type_declaration_table_merge_count: u64,
    type_declaration_id_copy_count: u64,
    type_declaration_entries_merged_total: u64,
    generated_default_lib_table_clone_count: u64,
    dependency_declaration_table_clone_count: u64,
    module_scope_cache_hits: u64,
    module_scope_cache_misses: u64,
    declaration_lookup_count: u64,
    declaration_lookup_layer_count_total: u64,
    expression_check_count: u64,
    expression_infer_count: u64,
    assignability_check_count: u64,
    property_lookup_count: u64,
    call_resolution_count: u64,
    generic_call_inference_attempt_count: u64,
    generic_call_inference_success_count: u64,
    generic_call_inference_failed_count: u64,
    generic_call_inference_explicit_type_args_skip_count: u64,
    generic_call_inference_unresolved_argument_skip_count: u64,
    generic_call_inference_tuple_return_suppressed_count: u64,
    generic_call_inference_candidate_count: u64,
    generic_indexed_access_attempt_count: u64,
    generic_indexed_access_substituted_receiver_count: u64,
    generic_indexed_access_substituted_key_count: u64,
    generic_indexed_access_success_count: u64,
    generic_indexed_access_unknown_fallback_count: u64,
    generic_indexed_access_invalid_key_count: u64,
    object_literal_property_check_count: u64,
    function_body_check_count: u64,
    type_declaration_lookup_count: u64,
    type_declaration_lookup_layer_steps_total: u64,
    type_clone_count: u64,
    object_type_clone_count: u64,
    object_type_id_copy_count: u64,
    union_type_clone_count: u64,
    symbol_name_clone_count: u64,
    string_key_clone_count: u64,
    flow_local_name_clone_count: u64,
    string_path_lookup_count: u64,
    canonical_file_id_lookup_count: u64,
    function_type_copy_from_expression_identifier_count: u64,
    function_type_copy_from_expression_call_return_count: u64,
    function_type_copy_from_expression_optional_call_return_count: u64,
    union_type_copy_from_expression_identifier_count: u64,
    union_type_copy_from_expression_call_return_count: u64,
    union_type_copy_from_expression_optional_call_return_count: u64,
    flow_function_count: u64,
    flow_function_skipped_count: u64,
    flow_statement_count: u64,
    flow_expression_visit_count: u64,
    flow_identifier_read_count: u64,
    flow_scope_push_count: u64,
    flow_scope_pop_count: u64,
    flow_future_declaration_collection_count: u64,
    flow_future_declaration_entries_total: u64,
    flow_state_clone_count: u64,
    flow_scope_locals_clone_count: u64,
    flow_state_full_clone_avoided_count: u64,
    flow_branch_merge_count: u64,
    flow_branch_merge_scope_count: u64,
    flow_branch_merge_local_iteration_count: u64,
    flow_branch_merge_fast_path_count: u64,
    flow_branch_empty_delta_count: u64,
    flow_branch_changed_local_count: u64,
    flow_read_lookup_count: u64,
    flow_read_lookup_scope_steps_total: u64,
    flow_return_analysis_walk_count: u64,
    flow_truthiness_check_count: u64,
    type_name_lookup_string_count: u64,
    symbol_info_handle_copy_count: u64,
    symbol_info_payload_deep_clone_count: u64,
    symbol_table_clone_count: u64,
    symbol_table_entry_handle_copy_count: u64,
    scope_stack_visible_rebuild_count: u64,
    scope_stack_visible_symbol_handle_copy_count: u64,
    module_export_table_clone_count: u64,
    module_export_entry_clone_count: u64,
    module_export_symbol_handle_copy_count: u64,
    module_export_borrowed_lookup_count: u64,
    module_export_namespace_export_object_materialization_count: u64,
    module_export_namespace_export_object_property_count: u64,
}

static PROGRAM_COUNTERS: OnceLock<Mutex<ProgramCounters>> = OnceLock::new();

#[derive(Debug, Default)]
pub(crate) struct ProgramTimings {
    pub(crate) parsing: Duration,
    pub(crate) ambient_collection: Duration,
    pub(crate) dependency_declaration_parse_time: Duration,
    pub(crate) dependency_declaration_lower_time: Duration,
    pub(crate) generated_default_lib_parse_time: Duration,
    pub(crate) generated_default_lib_lower_time: Duration,
    pub(crate) generated_default_lib_global_collection: Duration,
    pub(crate) root_source_global_collection: Duration,
    pub(crate) dependency_declaration_collection: Duration,
    pub(crate) type_declaration_collection: Duration,
    pub(crate) preliminary_module_type_binding_collection: Duration,
    pub(crate) module_analysis_collection: Duration,
    pub(crate) declaration_table_merging_cloning: Duration,
    pub(crate) utility_alias_validation: Duration,
    pub(crate) module_binding: Duration,
    pub(crate) preliminary_export_table_resolution: Duration,
    pub(crate) final_export_table_resolution: Duration,
    pub(crate) import_binding_resolution: Duration,
    pub(crate) import_specifier_resolution: Duration,
    pub(crate) export_table_lookup: Duration,
    pub(crate) re_export_expansion: Duration,
    pub(crate) package_export_lookup: Duration,
    pub(crate) type_binding_insert: Duration,
    pub(crate) value_binding_insert: Duration,
    pub(crate) module_resolution_scope_construction: Duration,
    pub(crate) ambient_module_binding: Duration,
    pub(crate) re_export_resolution: Duration,
    pub(crate) package_dependency_module_binding: Duration,
    pub(crate) generated_default_lib_module_handling: Duration,
    pub(crate) clone_copy_heavy_operations: Duration,
    pub(crate) declaration_validation: Duration,
    pub(crate) per_file_statement_checking: Duration,
    pub(crate) variable_declaration_checking: Duration,
    pub(crate) function_declaration_checking: Duration,
    pub(crate) expression_statement_checking: Duration,
    pub(crate) return_statement_checking: Duration,
    pub(crate) call_expression_checking: Duration,
    pub(crate) object_literal_checking: Duration,
    pub(crate) property_access_checking: Duration,
    pub(crate) assignability_checking: Duration,
    pub(crate) type_inference: Duration,
    pub(crate) flow_narrowing: Duration,
    pub(crate) file_metrics: HashMap<String, FileTimings>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FileTimings {
    pub(crate) collect_type_declarations_passes: u64,
    pub(crate) lowered_type_declarations: u64,
    pub(crate) validate_local_type_declarations_passes: u64,
    pub(crate) validate_local_type_declarations_items: u64,
    pub(crate) collect_type_declarations_duration: Duration,
    pub(crate) validate_local_type_declarations_duration: Duration,
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
    if files.is_empty() {
        return ProgramCheckResult {
            diagnostics: Vec::new(),
            stats: CompatibilityStats::default(),
        };
    }

    let mut files = files;
    inject_generated_default_lib_inputs(&mut files, options.no_lib);

    let timings_enabled = std::env::var_os("TYPESCRIPT_RUST_TIMINGS").is_some();
    let timings = timings_enabled.then(|| Arc::new(Mutex::new(ProgramTimings::default())));
    reset_program_counters();

    let parse_start = Instant::now();
    let parsed_files = parse_program_files(files, timings.as_ref());
    record_program_timing(timings.as_ref(), |timings| {
        timings.parsing += parse_start.elapsed()
    });
    let module_file_index_by_identity = parsed_files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            (
                canonicalize_if_exists_string(std::path::Path::new(&file.file_name)).into(),
                index,
            )
        })
        .collect::<HashMap<Arc<str>, usize>>();
    let file_kinds = parsed_files
        .iter()
        .map(|file| (file.file_name.clone(), file.file_kind))
        .collect::<HashMap<_, _>>();
    let first_file_name = parsed_files
        .first()
        .map(|file| file.file_name.clone())
        .unwrap_or_default();
    let mut ctx = CheckerContext::new(first_file_name, options, file_kinds);
    ctx.timings = timings.clone();
    ctx.set_module_file_index_by_identity(module_file_index_by_identity);

    crate::builtins::inject_builtins(&mut ctx);

    let mut global_symbols = SymbolTable::new();
    let mut function_signatures = HashMap::new();

    let ambient_collection_start = Instant::now();
    emit_parser_diagnostics(&parsed_files, &mut ctx);
    collect_ambient_globals(&parsed_files, &mut ctx, timings.as_ref());
    collect_ambient_modules(&parsed_files, &mut ctx, timings.as_ref());
    record_program_timing(timings.as_ref(), |timings| {
        timings.ambient_collection += ambient_collection_start.elapsed()
    });

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

    let type_collection_start = Instant::now();
    let preliminary_module_analyses = collect_module_analyses_with_bindings(
        &parsed_files,
        &local_type_declarations_by_module,
        &preliminary_module_import_bindings,
        &mut ctx,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_analysis_collection += type_collection_start.elapsed()
    });

    let local_module_export_tables = preliminary_module_analyses
        .iter()
        .map(|analysis| {
            analysis
                .as_ref()
                .map(|analysis| analysis.local_export_table.clone())
        })
        .collect::<Vec<_>>();
    let module_binding_start = Instant::now();
    let export_resolution_start = Instant::now();
    let module_export_tables =
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, &mut ctx);
    record_program_timing(timings.as_ref(), |timings| {
        timings.preliminary_export_table_resolution += export_resolution_start.elapsed()
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
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_declaration_collection_start.elapsed()
    });
    let import_binding_start = Instant::now();
    let module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &preliminary_module_analyses,
        &module_export_tables,
        &preliminary_module_resolution_scopes,
        &mut ctx,
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.import_binding_resolution += import_binding_start.elapsed()
    });
    let scope_build_start = Instant::now();
    let module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &module_import_bindings,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });
    let import_binding_start = Instant::now();
    let module_import_bindings = collect_module_import_bindings(
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
    let module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &module_import_bindings,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });
    let type_collection_start = Instant::now();
    let module_analyses = collect_module_analyses_with_bindings(
        &parsed_files,
        &local_type_declarations_by_module,
        &module_import_bindings,
        &mut ctx,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_analysis_collection += type_collection_start.elapsed()
    });
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_declaration_collection_start.elapsed()
    });
    let local_module_export_tables = module_analyses
        .iter()
        .map(|analysis| {
            analysis
                .as_ref()
                .map(|analysis| analysis.local_export_table.clone())
        })
        .collect::<Vec<_>>();
    let export_resolution_start = Instant::now();
    let module_export_tables =
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, &mut ctx);
    record_program_timing(timings.as_ref(), |timings| {
        timings.final_export_table_resolution += export_resolution_start.elapsed()
    });
    let import_binding_start = Instant::now();
    let module_import_bindings = collect_module_import_bindings(
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
    let module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &module_import_bindings,
        timings.as_ref(),
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_resolution_scope_construction += scope_build_start.elapsed()
    });
    sync_global_this_symbol(&mut ctx);
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_binding += module_binding_start.elapsed()
    });
    let shared_state = ProgramCheckSharedState {
        global_type_declarations,
        global_symbols,
        function_signatures,
        module_analyses,
        module_import_bindings: merge_module_import_bindings(
            &module_import_bindings,
            &preliminary_module_import_bindings,
        ),
        module_resolution_scopes,
    };

    let file_results = if jobs <= 1 || parsed_files.len() <= 1 {
        check_program_files_serial(&parsed_files, &shared_state, &ctx, timings.clone())
    } else {
        check_program_files_parallel(&parsed_files, &shared_state, &ctx, jobs, timings.clone())
    };

    for result in file_results {
        extend_diagnostics_dedup(&mut ctx.diagnostics, result.diagnostics);
        ctx.stats.suppressed_diagnostics_total += result.stats.suppressed_diagnostics_total;
        ctx.stats.suppressed_declaration_diagnostics_total +=
            result.stats.suppressed_declaration_diagnostics_total;
        ctx.stats.suppressed_rust_only_diagnostics_total +=
            result.stats.suppressed_rust_only_diagnostics_total;
    }

    let (diagnostics, stats) = ctx.finish_with_stats();

    if let Some(timings) = timings.as_ref() {
        render_program_timings(timings);
    }

    ProgramCheckResult { diagnostics, stats }
}

fn collect_preliminary_module_type_bindings(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> (
    Vec<Option<Arc<TypeDeclarationTable>>>,
    Vec<Option<ModuleImportBindings>>,
    Vec<Diagnostic>,
) {
    let mut local_type_declarations_by_module = Vec::with_capacity(parsed_files.len());
    let mut preliminary_local_export_tables = Vec::with_capacity(parsed_files.len());
    let mut preliminary_type_diagnostics = Vec::new();
    let initial_diagnostics_len = ctx.diagnostics().len();

    for parsed_file in parsed_files {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            local_type_declarations_by_module.push(None);
            preliminary_local_export_tables.push(None);
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;

        let diagnostics_before_collect = ctx.diagnostics().len();
        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());
        let collect_start = Instant::now();
        collect_type_declarations(&parsed_file.statements, ctx);
        let local_type_declarations = Arc::new(std::mem::take(&mut ctx.type_declarations));
        let lowered_type_declarations = local_type_declarations.len() as u64;
        let collect_duration = collect_start.elapsed();
        preliminary_type_diagnostics.extend(
            ctx.diagnostics()[diagnostics_before_collect..]
                .iter()
                .cloned(),
        );
        record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
            metrics.collect_type_declarations_passes += 1;
            metrics.lowered_type_declarations += lowered_type_declarations;
            metrics.collect_type_declarations_duration += collect_duration;
        });

        let preliminary_export_table = build_module_export_table(
            parsed_file,
            local_type_declarations.as_ref(),
            &SymbolTable::new(), // empty symbols
            None,
            ctx,
        );

        collect_global_augmentations_from_statements(&parsed_file.statements, ctx);
        ctx.type_declarations = saved_type_declarations;
        ctx.symbols = saved_symbols;
        ctx.type_declaration_scope = saved_type_declaration_scope;

        local_type_declarations_by_module.push(Some(local_type_declarations));
        preliminary_local_export_tables.push(Some(preliminary_export_table));
    }

    let preliminary_export_resolution_start = Instant::now();
    let preliminary_module_export_tables =
        resolve_module_export_tables(parsed_files, &preliminary_local_export_tables, ctx);
    record_program_timing(timings, |timings| {
        timings.preliminary_export_table_resolution += preliminary_export_resolution_start.elapsed()
    });

    let preliminary_scope_start = Instant::now();
    let preliminary_module_resolution_scopes = local_type_declarations_by_module
        .iter()
        .map(|local_type_declarations| {
            let Some(local_type_declarations) = local_type_declarations else {
                return None;
            };

            Some(Arc::new(TypeDeclarationScope::new(vec![
                local_type_declarations.clone(),
            ])))
        })
        .collect::<Vec<_>>();
    record_program_timing(timings, |timings| {
        timings.module_resolution_scope_construction += preliminary_scope_start.elapsed()
    });

    let mut preliminary_module_import_bindings = Vec::with_capacity(parsed_files.len());
    let preliminary_import_resolution_start = Instant::now();
    for (_file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            preliminary_module_import_bindings.push(None);
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let imported_bindings = resolve_module_imports(
            parsed_file,
            parsed_files,
            &preliminary_module_export_tables,
            &preliminary_module_resolution_scopes,
            &SymbolTable::new(), // empty local symbols
            ctx,
        );
        preliminary_module_import_bindings.push(Some(imported_bindings));
    }
    record_program_timing(timings, |timings| {
        timings.import_binding_resolution += preliminary_import_resolution_start.elapsed()
    });

    let replayable_preliminary_diagnostics = ctx.diagnostics()[initial_diagnostics_len..]
        .iter()
        .filter(|diagnostic| should_replay_preliminary_diagnostic(diagnostic, ctx))
        .cloned()
        .collect::<Vec<_>>();
    ctx.truncate_diagnostics(initial_diagnostics_len);
    preliminary_type_diagnostics.extend(replayable_preliminary_diagnostics);

    (
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
        preliminary_type_diagnostics,
    )
}

fn collect_module_analyses_with_bindings(
    parsed_files: &[ParsedProgramFile],
    local_type_declarations_by_module: &[Option<Arc<TypeDeclarationTable>>],
    preliminary_module_import_bindings: &[Option<ModuleImportBindings>],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> Vec<Option<ModuleAnalysis>> {
    let mut analyses = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            analyses.push(None);
            continue;
        }

        record_program_counter(|c| {
            c.module_analysis_total_calls += 1;
            c.module_analysis_unique_files += 1;
        });

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;
        let Some(local_type_declarations) = local_type_declarations_by_module[file_index].as_ref()
        else {
            analyses.push(None);
            ctx.type_declaration_scope = saved_type_declaration_scope;
            continue;
        };

        // Set up the full type environment for function signature collection
        let merge_start = Instant::now();
        let imported_type_declarations = preliminary_module_import_bindings[file_index]
            .as_ref()
            .map(|bindings| bindings.type_declarations.clone());
        let mut scope_layers = vec![local_type_declarations.clone()];
        if let Some(imported_type_declarations) = imported_type_declarations {
            scope_layers.push(imported_type_declarations);
        }
        let full_type_declarations_scope = Arc::new(TypeDeclarationScope::new(scope_layers));
        ctx.type_declarations = local_type_declarations.as_ref().clone();
        ctx.type_declaration_scope = Some(full_type_declarations_scope.clone());
        record_program_timing(timings, |timings| {
            timings.declaration_table_merging_cloning += merge_start.elapsed()
        });

        let mut local_symbols = SymbolTable::new();
        let mut local_function_signatures = HashMap::new();
        let diagnostics_before_signatures = ctx.diagnostics().len();
        collect_function_signatures_from_statements(
            &parsed_file.statements,
            file_index,
            &mut local_symbols,
            &mut local_function_signatures,
            ctx,
        );
        ctx.truncate_diagnostics(diagnostics_before_signatures);
        ctx.resolved_named_types = Arc::new(Mutex::new(HashMap::new()));

        let saved_symbols_for_global_augments =
            std::mem::replace(&mut ctx.symbols, local_symbols.clone());
        collect_global_augmentations_from_statements(&parsed_file.statements, ctx);
        ctx.symbols = saved_symbols_for_global_augments;

        let export_table = build_module_export_table(
            parsed_file,
            local_type_declarations.as_ref(),
            &local_symbols,
            Some(full_type_declarations_scope),
            ctx,
        );

        analyses.push(Some(ModuleAnalysis {
            local_type_declarations: local_type_declarations.clone(),
            local_symbols,
            local_function_signatures,
            local_export_table: export_table,
        }));
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }

    analyses
}

fn parse_program_files(
    files: Vec<SourceFileInput>,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> Vec<ParsedProgramFile> {
    files
        .into_iter()
        .map(|input| {
            let file_name = input.file_name;
            record_program_counter(|c| c.files_total += 1);
            if classify_file_kind(&file_name) == FileKind::GeneratedDeclaration {
                record_program_counter(|c| c.generated_default_lib_files += 1);
                return ParsedProgramFile {
                    file_name: file_name.clone(),
                    source_text: input.source_text,
                    statements: Vec::new(),
                    parser_errors: Vec::new(),
                    is_module: false,
                    file_kind: FileKind::GeneratedDeclaration,
                };
            }

            let parse_start = Instant::now();
            let parsed = parse_source(&input.source_text, &file_name);
            let parse_duration = parse_start.elapsed();
            let file_name = parsed.file_name;
            let file_kind = classify_file_kind(&file_name);
            record_program_timing(timings, |timings| match file_kind {
                FileKind::DependencyDeclaration => {
                    timings.dependency_declaration_parse_time += parse_duration
                }
                FileKind::GeneratedDeclaration => {
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
            });
            ParsedProgramFile {
                file_name: file_name.clone(),
                source_text: input.source_text,
                statements: parsed.statements,
                parser_errors: parsed.parser_errors,
                is_module: parsed.is_module,
                file_kind: classify_file_kind(&file_name),
            }
        })
        .collect()
}

fn build_module_resolution_scopes(
    local_type_declarations_by_module: &[Option<Arc<TypeDeclarationTable>>],
    module_import_bindings: &[Option<ModuleImportBindings>],
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> Vec<Option<Arc<TypeDeclarationScope>>> {
    local_type_declarations_by_module
        .iter()
        .enumerate()
        .map(|(file_index, local_type_declarations)| {
            let Some(local_type_declarations) = local_type_declarations else {
                return None;
            };

            let clone_start = Instant::now();
            let mut layers = vec![local_type_declarations.clone()];
            if let Some(imported) = module_import_bindings
                .get(file_index)
                .and_then(|bindings| bindings.as_ref())
            {
                layers.push(imported.type_declarations.clone());
            }
            record_program_timing(timings, |timings| {
                timings.clone_copy_heavy_operations += clone_start.elapsed()
            });
            Some(Arc::new(TypeDeclarationScope::new(layers)))
        })
        .collect()
}

fn classify_file_kind(file_name: &str) -> FileKind {
    if is_declaration_file_name(file_name) {
        if is_generated_declaration_file_name(file_name) {
            return FileKind::GeneratedDeclaration;
        }

        if file_name.contains("/node_modules/") || file_name.contains("/node_modules/.pnpm/") {
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

fn emit_parser_diagnostics(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());

        for message in &parsed_file.parser_errors {
            ctx.push(Diagnostic::typescript_rust_parser_error(
                message.clone(),
                parsed_file.file_name.clone(),
            ));
        }
    }
}

fn inject_generated_default_lib_inputs(files: &mut Vec<SourceFileInput>, no_lib: bool) {
    if no_lib
        || files
            .iter()
            .any(|file| crate::default_lib::is_generated_default_lib_file_name(&file.file_name))
    {
        return;
    }

    let mut default_lib_inputs = load_default_lib_inputs(false, None);
    if default_lib_inputs.is_empty() {
        return;
    }

    default_lib_inputs.extend(files.drain(..));
    *files = default_lib_inputs;
}

fn collect_global_type_declarations(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) {
    for parsed_file in parsed_files {
        if parsed_file.file_kind == FileKind::GeneratedDeclaration {
            continue;
        }

        if parsed_file.file_kind == FileKind::DependencyDeclaration || parsed_file.is_module {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let collect_start = Instant::now();
        collect_type_declarations(&parsed_file.statements, ctx);
        let lowered_type_declarations = ctx.type_declarations.len() as u64;
        let collect_duration = collect_start.elapsed();
        record_program_file_timing(timings, &parsed_file.file_name, |timings| {
            timings.collect_type_declarations_passes += 1;
            timings.lowered_type_declarations += lowered_type_declarations;
            timings.collect_type_declarations_duration += collect_duration
        });
    }
}

fn collect_global_function_signatures(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if parsed_file.file_kind == FileKind::GeneratedDeclaration {
            continue;
        }

        if parsed_file.is_module || parsed_file.file_kind.is_declaration() {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());

        collect_function_signatures_from_statements(
            &parsed_file.statements,
            file_index,
            global_symbols,
            function_signatures,
            ctx,
        );
    }
}

fn check_program_files_serial(
    parsed_files: &[ParsedProgramFile],
    shared_state: &ProgramCheckSharedState,
    ctx: &CheckerContext,
    timings: Option<Arc<Mutex<ProgramTimings>>>,
) -> Vec<FileCheckResult> {
    let mut results = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        let mut local_ctx = ctx.clone();
        local_ctx.diagnostics.clear();
        local_ctx.stats = CompatibilityStats::default();
        let result = check_program_file(
            file_index,
            parsed_file,
            shared_state,
            &mut local_ctx,
            timings.as_ref(),
        );
        results.push(result);
    }

    results
}

fn check_program_files_parallel(
    parsed_files: &[ParsedProgramFile],
    shared_state: &ProgramCheckSharedState,
    ctx: &CheckerContext,
    jobs: usize,
    timings: Option<Arc<Mutex<ProgramTimings>>>,
) -> Vec<FileCheckResult> {
    let worker_count = jobs.max(1).min(parsed_files.len());
    if worker_count <= 1 {
        return check_program_files_serial(parsed_files, shared_state, ctx, timings);
    }

    let chunk_size = (parsed_files.len() + worker_count - 1) / worker_count;
    let worker_base = ctx.clone();

    let results = thread::scope(|scope| {
        let mut handles = Vec::new();

        for (chunk_index, chunk) in parsed_files.chunks(chunk_size).enumerate() {
            let shared_state = shared_state;
            let worker_ctx = worker_base.clone();
            let timings = timings.clone();
            let start_index = chunk_index * chunk_size;

            handles.push(scope.spawn(move || {
                let mut local_ctx = worker_ctx;
                local_ctx.diagnostics.clear();
                local_ctx.stats = CompatibilityStats::default();

                let mut chunk_results = Vec::with_capacity(chunk.len());
                for (offset, parsed_file) in chunk.iter().enumerate() {
                    let file_index = start_index + offset;
                    chunk_results.push(check_program_file(
                        file_index,
                        parsed_file,
                        shared_state,
                        &mut local_ctx,
                        timings.as_ref(),
                    ));
                }

                chunk_results
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

    let mut flattened = results.into_iter().flatten().collect::<Vec<_>>();
    flattened.sort_by_key(|result| result.file_index);
    flattened
}

fn extend_diagnostics_dedup(
    diagnostics: &mut Vec<typescript_rust_diagnostics::Diagnostic>,
    new_diagnostics: Vec<typescript_rust_diagnostics::Diagnostic>,
) {
    for diagnostic in new_diagnostics {
        if diagnostics.iter().any(|existing| {
            existing.code.to_string() == diagnostic.code.to_string()
                && existing.file_name == diagnostic.file_name
                && existing.message == diagnostic.message
                && existing.span == diagnostic.span
        }) {
            continue;
        }

        diagnostics.push(diagnostic);
    }
}

fn check_program_file(
    file_index: usize,
    parsed_file: &ParsedProgramFile,
    shared_state: &ProgramCheckSharedState,
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> FileCheckResult {
    ctx.set_file_name(parsed_file.file_name.clone());
    ctx.type_declaration_scope = None;
    ctx.resolved_named_types = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));

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
                    layers.push(imported_bindings.type_declarations.clone());
                }
                record_module_scope_cache_miss();
                Arc::new(TypeDeclarationScope::new(layers))
            });
        if shared_state.module_resolution_scopes[file_index].is_some() {
            record_module_scope_cache_hit();
        }

        let mut merged_symbols = ctx
            .ambient_global_symbols
            .clone_with_reason(typescript_rust_types::TypeCopyReason::ScopeOrContext);
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
            merged_symbols.clone_with_reason(typescript_rust_types::TypeCopyReason::ScopeOrContext),
        );

        let current_type_declarations = ctx.type_declarations.clone();
        let current_symbols = ctx
            .symbols
            .clone_with_reason(typescript_rust_types::TypeCopyReason::ScopeOrContext);
        let validation_symbols = crate::modules::collect_exportable_value_symbols(
            &parsed_file.statements,
            &current_type_declarations,
            &current_symbols,
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

        ctx.symbols = saved_symbols;

        let mut signature_ctx = ctx.clone();
        signature_ctx.diagnostics.clear();
        signature_ctx.utility_diagnostic_keys.clear();
        signature_ctx.resolved_named_types =
            std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        let mut signature_local_symbols = crate::symbols::SymbolTable::new();
        for (name, symbol) in merged_symbols.iter_shared() {
            if !matches!(symbol.kind, crate::symbols::SymbolKind::Function) {
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

        let statement_check_start = Instant::now();
        check_program_file_statements(
            &parsed_file.statements,
            file_index,
            &final_function_signatures,
            ctx,
        );
        record_program_timing(timings, |timings| {
            timings.per_file_statement_checking += statement_check_start.elapsed()
        });
    } else {
        let mut script_td = clone_type_declaration_table(
            &shared_state.global_type_declarations,
            timings,
            TableCloneKind::General,
        );
        for (name, declaration) in ctx.ambient_global_type_declarations.iter() {
            let _ = script_td.insert(name.clone(), declaration.clone());
        }
        ctx.type_declarations = script_td;
        ctx.type_declaration_scope = None;

        let mut script_sym = shared_state
            .global_symbols
            .clone_with_reason(typescript_rust_types::TypeCopyReason::ScopeOrContext);
        for (name, symbol) in ctx.ambient_global_symbols.iter_handles() {
            let _ = script_sym.insert_handle(name.clone(), clone_symbol_info_handle(symbol));
        }
        ctx.set_symbols(script_sym);

        let current_type_declarations = ctx.type_declarations.clone();
        let current_symbols = ctx
            .symbols
            .clone_with_reason(typescript_rust_types::TypeCopyReason::ScopeOrContext);
        let validation_symbols = crate::modules::collect_exportable_value_symbols(
            &parsed_file.statements,
            &current_type_declarations,
            &current_symbols,
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

        ctx.symbols = saved_symbols;

        let statement_check_start = Instant::now();
        check_program_file_statements(
            &parsed_file.statements,
            file_index,
            &shared_state.function_signatures,
            ctx,
        );
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

fn collect_module_import_bindings(
    parsed_files: &[ParsedProgramFile],
    module_analyses: &[Option<ModuleAnalysis>],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleImportBindings>> {
    let mut module_import_bindings = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            module_import_bindings.push(None);
            continue;
        }

        let Some(module_analysis) = module_analyses[file_index].as_ref() else {
            module_import_bindings.push(None);
            continue;
        };

        ctx.set_file_name(parsed_file.file_name.clone());
        let imported_bindings = resolve_module_imports(
            parsed_file,
            parsed_files,
            module_export_tables,
            module_resolution_scopes,
            &module_analysis.local_symbols,
            ctx,
        );
        module_import_bindings.push(Some(imported_bindings));
    }

    module_import_bindings
}

fn should_replay_preliminary_diagnostic(diagnostic: &Diagnostic, ctx: &CheckerContext) -> bool {
    if diagnostic.code.to_string() != "TS2305" {
        return false;
    }

    let Some(module_specifier) = diagnostic
        .message
        .strip_prefix("Module '")
        .and_then(|message| {
            message
                .split_once("' has no exported member ")
                .map(|(module_specifier, _)| module_specifier)
        })
    else {
        return true;
    };

    ctx.ambient_modules.contains_key(module_specifier)
}

fn merge_module_import_bindings(
    final_bindings: &[Option<ModuleImportBindings>],
    fallback_bindings: &[Option<ModuleImportBindings>],
) -> Vec<Option<ModuleImportBindings>> {
    final_bindings
        .iter()
        .zip(fallback_bindings.iter())
        .map(|(final_bindings, fallback_bindings)| {
            let Some(final_bindings) = final_bindings else {
                return typescript_rust_types::with_type_copy_reason(
                    typescript_rust_types::TypeCopyReason::ModuleExport,
                    || fallback_bindings.clone(),
                );
            };

            let Some(fallback_bindings) = fallback_bindings else {
                return Some(
                    final_bindings
                        .clone_with_reason(typescript_rust_types::TypeCopyReason::ModuleExport),
                );
            };

            let mut merged_type_declarations = final_bindings.type_declarations.as_ref().clone();
            record_type_declaration_table_merge(
                None,
                fallback_bindings.type_declarations.len(),
                TableMergeKind::General,
            );
            for (name, declaration) in fallback_bindings.type_declarations.iter() {
                if merged_type_declarations.get(name.as_ref()).is_none() {
                    let _ = merged_type_declarations.insert(name.clone(), declaration.clone());
                }
            }
            let mut merged_bindings = final_bindings
                .clone_with_reason(typescript_rust_types::TypeCopyReason::ModuleExport);
            merged_bindings.type_declarations = Arc::new(merged_type_declarations);
            for (name, symbol) in fallback_bindings.symbols.iter_shared() {
                if merged_bindings.symbols.get(name).is_none() {
                    let _ = merged_bindings
                        .symbols
                        .insert_shared(name.clone(), symbol.clone());
                }
            }

            Some(merged_bindings)
        })
        .collect()
}

pub(crate) fn collect_function_signatures_from_statements(
    statements: &[ParsedStatement],
    file_index: usize,
    symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (statement_index, statement) in statements.iter().enumerate() {
        collect_function_signature_from_statement(
            statement,
            file_index,
            statement_index,
            symbols,
            function_signatures,
            ctx,
        );
    }
}

fn collect_function_signature_from_statement(
    statement: &ParsedStatement,
    file_index: usize,
    statement_index: usize,
    symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::FunctionDeclaration(function) => {
            let function_type =
                check_function::collect_function_declaration_signature(function, symbols, ctx);
            function_signatures.insert(
                FunctionDeclarationLocation {
                    file_index,
                    statement_index,
                },
                function_type,
            );
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_function_signature_from_statement(
            declaration.as_ref(),
            file_index,
            statement_index,
            symbols,
            function_signatures,
            ctx,
        ),
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration: ParsedDefaultExportDeclaration::Function(function),
            ..
        }) => {
            let function_type =
                check_function::collect_function_declaration_signature(function, symbols, ctx);
            function_signatures.insert(
                FunctionDeclarationLocation {
                    file_index,
                    statement_index,
                },
                function_type,
            );
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default { .. }) => {}
        _ => {}
    }
}

fn check_program_file_statements(
    statements: &[ParsedStatement],
    file_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (statement_index, statement) in statements.iter().cloned().enumerate() {
        check_program_statement(
            statement,
            file_index,
            statement_index,
            function_signatures,
            ctx,
        );
    }
}

fn check_program_statement(
    statement: ParsedStatement,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            let start = Instant::now();
            var::check_variable_declaration(variable, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.variable_declaration_checking += start.elapsed()
            });
        }
        ParsedStatement::Assignment(assignment) => {
            assign::check_assignment(assignment, ctx);
        }
        ParsedStatement::FunctionDeclaration(function) => {
            check_program_function_declaration(
                function,
                file_index,
                statement_index,
                function_signatures,
                ctx,
            );
        }
        ParsedStatement::Call(call) => {
            call::check_call(call, ctx);
        }
        ParsedStatement::Expression(expression) => {
            expr::check_expression_statement(expression, ctx);
        }
        ParsedStatement::TypeAliasDeclaration(_) => {}
        ParsedStatement::InterfaceDeclaration(_) => {}
        ParsedStatement::ImportDeclaration(_) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => check_program_statement(
            *declaration,
            file_index,
            statement_index,
            function_signatures,
            ctx,
        ),
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Namespace { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration,
            ..
        }) => match declaration {
            ParsedDefaultExportDeclaration::Function(function) => {
                check_program_function_declaration(
                    function,
                    file_index,
                    statement_index,
                    function_signatures,
                    ctx,
                );
            }
            ParsedDefaultExportDeclaration::Expression(expression) => {
                expr::check_expression_statement(expression, ctx);
            }
            ParsedDefaultExportDeclaration::Class { .. } => {}
            ParsedDefaultExportDeclaration::Unsupported { span } => {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        },
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Empty { .. }) => {}
        ParsedStatement::DeclareModuleDeclaration(_) => {}
        ParsedStatement::UnsupportedDeclaration { span } => {
            emit_unsupported_declaration_diagnostic(ctx, span);
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { span }) => {
            let mut diagnostic =
                Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

            if let Some(span) = span {
                diagnostic = diagnostic.with_span(crate::context::convert_span(span));
            }

            ctx.push(diagnostic);
        }
    }
}

fn emit_unsupported_declaration_diagnostics(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        emit_unsupported_declaration_diagnostic_from_statement(statement, ctx);
    }
}

fn emit_unsupported_declaration_diagnostic_from_statement(
    statement: &ParsedStatement,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::UnsupportedDeclaration { span } => {
            emit_unsupported_declaration_diagnostic(ctx, *span);
        }
        ParsedStatement::ImportDeclaration(import)
            if matches!(import.kind, ParsedImportKind::Unsupported) =>
        {
            emit_unsupported_declaration_diagnostic(
                ctx,
                import.span.or(import.module_specifier_span),
            );
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { span }) => {
            emit_unsupported_declaration_diagnostic(ctx, *span);
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration: ParsedDefaultExportDeclaration::Unsupported { span },
            span: declaration_span,
        }) => {
            emit_unsupported_declaration_diagnostic(ctx, (*span).or(*declaration_span));
        }
        ParsedStatement::DeclareModuleDeclaration(module) => {
            emit_unsupported_declaration_diagnostics(&module.statements, ctx);
        }
        _ => {}
    }
}

fn emit_unsupported_declaration_diagnostic(ctx: &mut CheckerContext, span: Option<TextSpan>) {
    let mut diagnostic = Diagnostic::typescript_rust_unsupported_declaration(ctx.file_name.clone());

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
    }

    ctx.push(diagnostic);
}

fn check_program_function_declaration(
    function: ParsedFunctionDeclaration,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    let declaration_location = FunctionDeclarationLocation {
        file_index,
        statement_index,
    };

    let saved_symbols = std::mem::take(&mut ctx.symbols);
    let body_root_symbols =
        saved_symbols.clone_with_reason(typescript_rust_types::TypeCopyReason::FunctionBodySetup);
    ctx.symbols = body_root_symbols;
    let Some(function_type) = function_signatures.get(&declaration_location) else {
        check_function::check_function_declaration(function, ctx);
        ctx.symbols = saved_symbols;
        return;
    };

    let type_parameters = function.type_parameters.clone();
    check_function::check_function_declaration_body(function, function_type, &type_parameters, ctx);
    ctx.symbols = saved_symbols;
}

fn count_local_type_declarations_in_statements(statements: &[ParsedStatement]) -> usize {
    statements
        .iter()
        .map(count_local_type_declarations_in_statement)
        .sum()
}

fn count_local_type_declarations_in_statement(statement: &ParsedStatement) -> usize {
    match statement {
        ParsedStatement::TypeAliasDeclaration(_) => 1,
        ParsedStatement::InterfaceDeclaration(_) => 1,
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => count_local_type_declarations_in_statement(declaration.as_ref()),
        _ => 0,
    }
}

fn collect_ambient_globals(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) {
    for parsed_file in parsed_files {
        if !parsed_file.file_kind.is_declaration() {
            continue;
        }

        if parsed_file.file_kind == FileKind::GeneratedDeclaration {
            ctx.set_file_name(parsed_file.file_name.clone());
            let _ = inject_generated_default_lib_snapshot_for_file_name(
                &parsed_file.file_name,
                ctx,
                timings,
            );
            continue;
        }

        if parsed_file.file_kind == FileKind::DependencyDeclaration {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;

        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        let collect_start = Instant::now();
        collect_type_declarations(&parsed_file.statements, ctx);
        let ambient_td = std::mem::take(&mut ctx.type_declarations);
        let lowered_type_declarations = ambient_td.len() as u64;
        let collect_duration = collect_start.elapsed();
        record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
            metrics.collect_type_declarations_passes += 1;
            metrics.lowered_type_declarations += lowered_type_declarations;
            metrics.collect_type_declarations_duration += collect_duration;
        });
        record_program_timing(timings, |timings| {
            if parsed_file.file_kind == FileKind::GeneratedDeclaration {
                timings.generated_default_lib_global_collection += collect_duration;
            } else {
                timings.dependency_declaration_collection += collect_duration;
                timings.dependency_declaration_lower_time += collect_duration;
            }
        });

        for (name, decl) in ambient_td.iter() {
            let _ = ctx
                .ambient_global_type_declarations
                .insert(name.clone(), decl.clone());
        }

        let mut local_function_signatures = HashMap::new();
        let mut current_symbols = std::mem::take(&mut ctx.symbols);
        collect_function_signatures_from_statements(
            &parsed_file.statements,
            0,
            &mut current_symbols,
            &mut local_function_signatures,
            ctx,
        );
        ctx.symbols = current_symbols;

        for stmt in &parsed_file.statements {
            let var = match stmt {
                ParsedStatement::VariableDeclaration(var) => Some(var),
                ParsedStatement::ExportDeclaration(
                    typescript_rust_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    },
                ) => {
                    if let ParsedStatement::VariableDeclaration(var) = declaration.as_ref() {
                        Some(var)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(var) = var {
                let ty = var
                    .declared_type
                    .as_ref()
                    .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                    .unwrap_or(typescript_rust_types::Type::Unknown);
                if ctx.ambient_global_symbols.get(&var.name).is_none() {
                    ctx.ambient_global_symbols.insert(
                        var.name.clone(),
                        crate::symbols::SymbolInfo {
                            ty,
                            kind: if matches!(
                                var.kind,
                                typescript_rust_syntax::ParsedVariableKind::Const
                            ) {
                                crate::symbols::SymbolKind::Const
                            } else {
                                crate::symbols::SymbolKind::Let
                            },
                            function_signature: None,
                        },
                    );
                }
            }
        }

        for (loc, fun_ty) in local_function_signatures {
            let name = match &parsed_file.statements[loc.statement_index] {
                ParsedStatement::FunctionDeclaration(f) => f.name.clone(),
                ParsedStatement::ExportDeclaration(
                    typescript_rust_syntax::ParsedExportDeclaration::Default {
                        declaration:
                            typescript_rust_syntax::ParsedDefaultExportDeclaration::Function(f),
                        ..
                    },
                ) => f.name.clone(),
                ParsedStatement::ExportDeclaration(
                    typescript_rust_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    },
                ) => {
                    if let ParsedStatement::FunctionDeclaration(f) = declaration.as_ref() {
                        f.name.clone()
                    } else {
                        "unknown".to_string()
                    }
                }
                _ => "unknown".to_string(),
            };

            if ctx.ambient_global_symbols.get(&name).is_none() {
                ctx.ambient_global_symbols.insert(
                    name,
                    crate::symbols::SymbolInfo {
                        ty: typescript_rust_types::Type::Function(fun_ty),
                        kind: crate::symbols::SymbolKind::Function,
                        function_signature: None,
                    },
                );
            }
        }

        ctx.type_declarations = saved_type_declarations;
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }
}

fn collect_ambient_modules(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) {
    let ambient_binding_start = Instant::now();
    let mut ambient_module_entries = Vec::<AmbientModuleEntry>::new();
    let mut ambient_module_indexes = HashMap::<String, usize>::new();

    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;
        for statement in &parsed_file.statements {
            let ParsedStatement::DeclareModuleDeclaration(module) = statement else {
                continue;
            };

            if module.module_specifier == "global" {
                continue;
            }

            let saved_type_declarations =
                std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
            let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());

            let collect_start = Instant::now();
            collect_type_declarations(&module.statements, ctx);
            let mut local_function_signatures = HashMap::new();
            let mut current_symbols = std::mem::take(&mut ctx.symbols);
            collect_function_signatures_from_statements(
                &module.statements,
                0,
                &mut current_symbols,
                &mut local_function_signatures,
                ctx,
            );
            ctx.symbols = current_symbols;

            for stmt in &module.statements {
                match stmt {
                    ParsedStatement::VariableDeclaration(var) => {
                        if var.is_declare && ctx.symbols.get(&var.name).is_none() {
                            let ty = var
                                .declared_type
                                .as_ref()
                                .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                                .unwrap_or(typescript_rust_types::Type::Unknown);
                            ctx.symbols.insert(
                                var.name.clone(),
                                crate::symbols::SymbolInfo {
                                    kind: if matches!(
                                        var.kind,
                                        typescript_rust_syntax::ParsedVariableKind::Const
                                    ) {
                                        crate::symbols::SymbolKind::Const
                                    } else {
                                        crate::symbols::SymbolKind::Let
                                    },
                                    ty,
                                    function_signature: None,
                                },
                            );
                        }
                    }
                    ParsedStatement::ExportDeclaration(
                        typescript_rust_syntax::ParsedExportDeclaration::Statement {
                            declaration,
                            ..
                        },
                    ) => {
                        if let ParsedStatement::VariableDeclaration(var) = declaration.as_ref() {
                            if ctx.symbols.get(&var.name).is_none() {
                                let ty = var
                                    .declared_type
                                    .as_ref()
                                    .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                                    .unwrap_or(typescript_rust_types::Type::Unknown);
                                ctx.symbols.insert(
                                    var.name.clone(),
                                    crate::symbols::SymbolInfo {
                                        kind: if matches!(
                                            var.kind,
                                            typescript_rust_syntax::ParsedVariableKind::Const
                                        ) {
                                            crate::symbols::SymbolKind::Const
                                        } else {
                                            crate::symbols::SymbolKind::Let
                                        },
                                        ty,
                                        function_signature: None,
                                    },
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }

            let mut temp_file = parsed_file.clone();
            temp_file.statements = module.statements.clone();
            let current_type_declarations = std::mem::take(&mut ctx.type_declarations);
            let current_symbols = std::mem::take(&mut ctx.symbols);
            record_type_declaration_table_clone(
                timings,
                current_type_declarations.len(),
                TableCloneKind::General,
            );
            let current_type_declarations_scope =
                Arc::new(TypeDeclarationScope::new(vec![Arc::new(
                    current_type_declarations.clone(),
                )]));
            let raw_export_table = build_module_export_table(
                &temp_file,
                &current_type_declarations,
                &current_symbols,
                Some(current_type_declarations_scope),
                ctx,
            );
            let lowered_type_declarations = current_type_declarations.len() as u64;
            ctx.type_declarations = current_type_declarations;
            ctx.symbols = current_symbols;

            if let Some(existing_index) = ambient_module_indexes
                .get(&module.module_specifier)
                .copied()
            {
                merge_module_export_tables(
                    &mut ambient_module_entries[existing_index].raw_export_table,
                    &raw_export_table,
                );
                if let Some(existing_table) = ctx.ambient_modules.get_mut(&module.module_specifier)
                {
                    merge_module_export_tables(existing_table, &raw_export_table);
                }
            } else {
                ctx.ambient_modules
                    .insert(module.module_specifier.clone(), raw_export_table.clone());
                ambient_module_indexes.insert(
                    module.module_specifier.clone(),
                    ambient_module_entries.len(),
                );
                ambient_module_entries.push(AmbientModuleEntry {
                    module_specifier: module.module_specifier.clone(),
                    file: temp_file,
                    raw_export_table,
                });
            }

            ctx.type_declarations = saved_type_declarations;
            ctx.symbols = saved_symbols;
            let collect_duration = collect_start.elapsed();
            record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
                metrics.collect_type_declarations_passes += 1;
                metrics.lowered_type_declarations += lowered_type_declarations;
                metrics.collect_type_declarations_duration += collect_duration;
            });
        }
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }

    if ambient_module_entries.is_empty() {
        return;
    }

    let ambient_files = ambient_module_entries
        .iter()
        .map(|entry| entry.file.clone())
        .collect::<Vec<_>>();
    let local_module_export_tables = ambient_module_entries
        .iter()
        .map(|entry| Some(entry.raw_export_table.clone()))
        .collect::<Vec<_>>();

    let mut resolved_module_export_tables = vec![None; ambient_module_entries.len()];
    let mut resolving = vec![false; ambient_module_entries.len()];

    for (file_index, entry) in ambient_module_entries.iter().enumerate() {
        if let Some(resolved_export_table) = crate::modules::resolve_module_export_table(
            file_index,
            &ambient_files,
            &local_module_export_tables,
            &mut resolved_module_export_tables,
            &mut resolving,
            ctx,
        ) {
            ctx.ambient_modules
                .insert(entry.module_specifier.clone(), resolved_export_table);
        }
    }

    record_program_timing(timings, |timings| {
        timings.ambient_module_binding += ambient_binding_start.elapsed()
    });
}

fn merge_module_export_tables(target: &mut ModuleExportTable, source: &ModuleExportTable) {
    record_type_declaration_table_merge(
        None,
        source.type_declarations.len(),
        TableMergeKind::General,
    );
    for (name, declaration) in source.type_declarations.iter() {
        if target.type_declarations.get(name.as_ref()).is_none() {
            let _ = target
                .type_declarations
                .insert(name.clone(), declaration.clone());
        }
    }

    for (name, symbol) in source.symbols.iter_shared() {
        if target.symbols.get(name).is_none() {
            let _ = target.symbols.insert_shared(name.clone(), symbol.clone());
        }
    }

    if target.default_symbol.is_none() {
        target.default_symbol = source.default_symbol.clone();
    }

    target.namespace_export_object_type = None;
    target.has_unresolved_star_export |= source.has_unresolved_star_export;
    target.has_incomplete_declaration_surface |= source.has_incomplete_declaration_surface;
}

fn collect_global_variables(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    for parsed_file in parsed_files {
        if parsed_file.file_kind == FileKind::GeneratedDeclaration {
            continue;
        }

        if parsed_file.file_kind == FileKind::DependencyDeclaration || parsed_file.is_module {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        for statement in &parsed_file.statements {
            let var = match statement {
                ParsedStatement::VariableDeclaration(var) => Some(var),
                ParsedStatement::ExportDeclaration(
                    typescript_rust_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    },
                ) => {
                    if let ParsedStatement::VariableDeclaration(var) = declaration.as_ref() {
                        Some(var)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(var) = var {
                if var.is_declare || parsed_file.file_kind.is_declaration() {
                    let ty = var
                        .declared_type
                        .as_ref()
                        .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                        .unwrap_or(typescript_rust_types::Type::Unknown);
                    if !parsed_file.file_kind.is_declaration()
                        || global_symbols.get(&var.name).is_none()
                    {
                        global_symbols.insert(
                            var.name.clone(),
                            crate::symbols::SymbolInfo {
                                kind: if matches!(
                                    var.kind,
                                    typescript_rust_syntax::ParsedVariableKind::Const
                                ) {
                                    crate::symbols::SymbolKind::Const
                                } else {
                                    crate::symbols::SymbolKind::Let
                                },
                                ty,
                                function_signature: None,
                            },
                        );
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) fn collect_local_value_symbols(
    statements: &[ParsedStatement],
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        collect_local_value_symbols_from_statement(statement, symbols, ctx);
    }
}

#[allow(dead_code)]
fn collect_local_value_symbols_from_statement(
    statement: &ParsedStatement,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(var) => {
            if var.is_declare {
                return;
            }

            let symbol_kind =
                if matches!(var.kind, typescript_rust_syntax::ParsedVariableKind::Const) {
                    crate::symbols::SymbolKind::Const
                } else {
                    crate::symbols::SymbolKind::Let
                };

            let ty = if let Some(declared_type) = var.declared_type.as_ref() {
                crate::infer::map_parsed_type(declared_type.clone(), ctx)
            } else if let Some(initializer) = var.initializer.as_ref() {
                let inferred =
                    expr::evaluate_expression(initializer, var.initializer_span, symbols, ctx);

                match inferred {
                    crate::infer::InferredExpression::Known(inferred_ty)
                        if inferred_ty != typescript_rust_types::Type::Unknown =>
                    {
                        var::widen_implicit_variable_initializer_type(symbol_kind, &inferred_ty)
                    }
                    _ => typescript_rust_types::Type::Unknown,
                }
            } else {
                typescript_rust_types::Type::Unknown
            };

            symbols.insert(
                var.name.clone(),
                crate::symbols::SymbolInfo {
                    ty,
                    kind: symbol_kind,
                    function_signature: None,
                },
            );
        }
        ParsedStatement::ExportDeclaration(
            typescript_rust_syntax::ParsedExportDeclaration::Statement { declaration, .. },
        ) => collect_local_value_symbols_from_statement(declaration.as_ref(), symbols, ctx),
        _ => {}
    }
}

pub(crate) fn record_program_timing(
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    update: impl FnOnce(&mut ProgramTimings),
) {
    let Some(timings) = timings else {
        return;
    };

    if let Ok(mut guard) = timings.lock() {
        update(&mut guard);
    }
}

pub(crate) fn record_program_file_timing(
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    file_name: &str,
    update: impl FnOnce(&mut FileTimings),
) {
    let Some(timings) = timings else {
        return;
    };

    if let Ok(mut guard) = timings.lock() {
        update(guard.file_metrics.entry(file_name.to_string()).or_default());
    }
}

fn render_program_timings(timings: &Arc<Mutex<ProgramTimings>>) {
    let Ok(timings) = timings.lock() else {
        return;
    };

    eprintln!("Timings:");
    eprintln!("  parsing: {}", format_duration(timings.parsing));
    eprintln!(
        "  ambient_collection: {}",
        format_duration(timings.ambient_collection)
    );
    eprintln!(
        "  dependency_declaration_parse_time: {}",
        format_duration(timings.dependency_declaration_parse_time)
    );
    eprintln!(
        "  dependency_declaration_lower_time: {}",
        format_duration(timings.dependency_declaration_lower_time)
    );
    eprintln!(
        "  generated_default_lib_parse_time: {}",
        format_duration(timings.generated_default_lib_parse_time)
    );
    eprintln!(
        "  generated_default_lib_lower_time: {}",
        format_duration(timings.generated_default_lib_lower_time)
    );
    eprintln!(
        "  generated_default_lib_global_collection: {}",
        format_duration(timings.generated_default_lib_global_collection)
    );
    eprintln!(
        "  root_source_global_collection: {}",
        format_duration(timings.root_source_global_collection)
    );
    eprintln!(
        "  dependency_declaration_collection: {}",
        format_duration(timings.dependency_declaration_collection)
    );
    eprintln!(
        "  type_declaration_collection: {}",
        format_duration(timings.type_declaration_collection)
    );
    eprintln!(
        "    preliminary_module_type_binding_collection: {}",
        format_duration(timings.preliminary_module_type_binding_collection)
    );
    eprintln!(
        "    module_analysis_collection: {}",
        format_duration(timings.module_analysis_collection)
    );
    eprintln!(
        "    declaration_table_merging_cloning: {}",
        format_duration(timings.declaration_table_merging_cloning)
    );
    eprintln!(
        "    utility_alias_validation: {}",
        format_duration(timings.utility_alias_validation)
    );
    eprintln!(
        "  module_binding: {}",
        format_duration(timings.module_binding)
    );
    eprintln!(
        "    preliminary_export_table_resolution: {}",
        format_duration(timings.preliminary_export_table_resolution)
    );
    eprintln!(
        "    final_export_table_resolution: {}",
        format_duration(timings.final_export_table_resolution)
    );
    eprintln!(
        "    import_binding_resolution: {}",
        format_duration(timings.import_binding_resolution)
    );
    eprintln!(
        "      import_specifier_resolution: {}",
        format_duration(timings.import_specifier_resolution)
    );
    eprintln!(
        "      export_table_lookup: {}",
        format_duration(timings.export_table_lookup)
    );
    eprintln!(
        "      re_export_expansion: {}",
        format_duration(timings.re_export_expansion)
    );
    eprintln!(
        "      package_export_lookup: {}",
        format_duration(timings.package_export_lookup)
    );
    eprintln!(
        "      type_binding_insert: {}",
        format_duration(timings.type_binding_insert)
    );
    eprintln!(
        "      value_binding_insert: {}",
        format_duration(timings.value_binding_insert)
    );
    eprintln!(
        "    module_resolution_scope_construction: {}",
        format_duration(timings.module_resolution_scope_construction)
    );
    eprintln!(
        "    ambient_module_binding: {}",
        format_duration(timings.ambient_module_binding)
    );
    eprintln!(
        "    re_export_resolution: {}",
        format_duration(timings.re_export_resolution)
    );
    eprintln!(
        "    package_dependency_module_binding: {}",
        format_duration(timings.package_dependency_module_binding)
    );
    eprintln!(
        "    generated_default_lib_module_handling: {}",
        format_duration(timings.generated_default_lib_module_handling)
    );
    eprintln!(
        "    clone_copy_heavy_operations: {}",
        format_duration(timings.clone_copy_heavy_operations)
    );
    eprintln!(
        "  declaration_validation: {}",
        format_duration(timings.declaration_validation)
    );
    eprintln!(
        "  per_file_statement_checking: {}",
        format_duration(timings.per_file_statement_checking)
    );
    eprintln!(
        "    variable_declaration_checking: {}",
        format_duration(timings.variable_declaration_checking)
    );
    eprintln!(
        "    function_declaration_checking: {}",
        format_duration(timings.function_declaration_checking)
    );
    eprintln!(
        "    expression_statement_checking: {}",
        format_duration(timings.expression_statement_checking)
    );
    eprintln!(
        "    return_statement_checking: {}",
        format_duration(timings.return_statement_checking)
    );
    eprintln!(
        "    call_expression_checking: {}",
        format_duration(timings.call_expression_checking)
    );
    eprintln!(
        "    object_literal_checking: {}",
        format_duration(timings.object_literal_checking)
    );
    eprintln!(
        "    property_access_checking: {}",
        format_duration(timings.property_access_checking)
    );
    eprintln!(
        "    assignability_checking: {}",
        format_duration(timings.assignability_checking)
    );
    eprintln!(
        "    type_inference: {}",
        format_duration(timings.type_inference)
    );
    eprintln!(
        "    flow_narrowing: {}",
        format_duration(timings.flow_narrowing)
    );
    if !timings.file_metrics.is_empty() {
        let mut file_metrics = timings.file_metrics.iter().collect::<Vec<_>>();
        file_metrics.sort_by(|(file_a, metrics_a), (file_b, metrics_b)| {
            metrics_b
                .collect_type_declarations_passes
                .cmp(&metrics_a.collect_type_declarations_passes)
                .then_with(|| file_a.cmp(file_b))
        });

        eprintln!("  file_metrics:");
        for (file_name, metrics) in file_metrics {
            eprintln!(
                "    {} | collect_type_declarations={} lower_type_declarations={} validate_local_type_declarations={} | collect_time={} validate_time={}",
                file_name,
                metrics.collect_type_declarations_passes,
                metrics.lowered_type_declarations,
                metrics.validate_local_type_declarations_passes,
                format_duration(metrics.collect_type_declarations_duration),
                format_duration(metrics.validate_local_type_declarations_duration)
            );
        }
    }

    let counters = snapshot_program_counters();
    eprintln!("  counters:");
    eprintln!("    files_total: {}", counters.files_total);
    eprintln!("    root_source_files: {}", counters.root_source_files);
    eprintln!(
        "    dependency_declaration_files: {}",
        counters.dependency_declaration_files
    );
    eprintln!(
        "    generated_default_lib_files: {}",
        counters.generated_default_lib_files
    );
    eprintln!(
        "    parsed_root_source_files: {}",
        counters.parsed_root_source_files
    );
    eprintln!(
        "    parsed_dependency_declaration_files: {}",
        counters.parsed_dependency_declaration_files
    );
    eprintln!(
        "    parsed_generated_default_lib_files: {}",
        counters.parsed_generated_default_lib_files
    );
    eprintln!(
        "    checker_arena_alloc_count: {}",
        counters.checker_arena_alloc_count
    );
    eprintln!(
        "    arena_declaration_key_alloc_count: {}",
        counters.arena_declaration_key_alloc_count
    );
    eprintln!(
        "    arena_type_declaration_payload_alloc_count: {}",
        counters.arena_type_declaration_payload_alloc_count
    );
    eprintln!(
        "    arena_object_type_payload_alloc_count: {}",
        counters.arena_object_type_payload_alloc_count
    );
    eprintln!(
        "    type_declaration_payload_deep_clone_count: {}",
        counters.type_declaration_payload_deep_clone_count
    );
    eprintln!(
        "    object_type_payload_deep_clone_count: {}",
        counters.object_type_payload_deep_clone_count
    );
    eprintln!(
        "    object_type_alloc_count: {}",
        counters.object_type_alloc_count
    );
    eprintln!(
        "    union_type_alloc_count: {}",
        counters.union_type_alloc_count
    );
    eprintln!(
        "    function_type_alloc_count: {}",
        counters.function_type_alloc_count
    );
    eprintln!(
        "    module_analysis_total_calls: {}",
        counters.module_analysis_total_calls
    );
    eprintln!(
        "    module_analysis_unique_files: {}",
        counters.module_analysis_unique_files
    );
    eprintln!(
        "    module_analysis_duplicate_calls: {}",
        counters.module_analysis_duplicate_calls
    );
    eprintln!(
        "    type_declaration_table_clone_count: {}",
        counters.type_declaration_table_clone_count
    );
    eprintln!(
        "    type_declaration_table_merge_count: {}",
        counters.type_declaration_table_merge_count
    );
    eprintln!(
        "    type_declaration_id_copy_count: {}",
        counters.type_declaration_id_copy_count
    );
    eprintln!(
        "    type_declaration_entries_merged_total: {}",
        counters.type_declaration_entries_merged_total
    );
    eprintln!(
        "    generated_default_lib_table_clone_count: {}",
        counters.generated_default_lib_table_clone_count
    );
    eprintln!(
        "    dependency_declaration_table_clone_count: {}",
        counters.dependency_declaration_table_clone_count
    );
    eprintln!(
        "    module_scope_cache_hits: {}",
        counters.module_scope_cache_hits
    );
    eprintln!(
        "    module_scope_cache_misses: {}",
        counters.module_scope_cache_misses
    );
    eprintln!(
        "    declaration_lookup_count: {}",
        counters.declaration_lookup_count
    );
    let declaration_lookup_avg = if counters.declaration_lookup_count == 0 {
        0.0
    } else {
        counters.declaration_lookup_layer_count_total as f64
            / counters.declaration_lookup_count as f64
    };
    eprintln!(
        "    declaration_lookup_layer_count_avg: {:.2}",
        declaration_lookup_avg
    );
    eprintln!(
        "    expression_check_count: {}",
        counters.expression_check_count
    );
    eprintln!(
        "    expression_infer_count: {}",
        counters.expression_infer_count
    );
    eprintln!(
        "    assignability_check_count: {}",
        counters.assignability_check_count
    );
    eprintln!(
        "    property_lookup_count: {}",
        counters.property_lookup_count
    );
    eprintln!(
        "    call_resolution_count: {}",
        counters.call_resolution_count
    );
    eprintln!(
        "    generic_call_inference_attempt_count: {}",
        counters.generic_call_inference_attempt_count
    );
    eprintln!(
        "    generic_call_inference_success_count: {}",
        counters.generic_call_inference_success_count
    );
    eprintln!(
        "    generic_call_inference_failed_count: {}",
        counters.generic_call_inference_failed_count
    );
    eprintln!(
        "    generic_call_inference_explicit_type_args_skip_count: {}",
        counters.generic_call_inference_explicit_type_args_skip_count
    );
    eprintln!(
        "    generic_call_inference_unresolved_argument_skip_count: {}",
        counters.generic_call_inference_unresolved_argument_skip_count
    );
    eprintln!(
        "    generic_call_inference_tuple_return_suppressed_count: {}",
        counters.generic_call_inference_tuple_return_suppressed_count
    );
    eprintln!(
        "    generic_call_inference_candidate_count: {}",
        counters.generic_call_inference_candidate_count
    );
    eprintln!(
        "    generic_indexed_access_attempt_count: {}",
        counters.generic_indexed_access_attempt_count
    );
    eprintln!(
        "    generic_indexed_access_substituted_receiver_count: {}",
        counters.generic_indexed_access_substituted_receiver_count
    );
    eprintln!(
        "    generic_indexed_access_substituted_key_count: {}",
        counters.generic_indexed_access_substituted_key_count
    );
    eprintln!(
        "    generic_indexed_access_success_count: {}",
        counters.generic_indexed_access_success_count
    );
    eprintln!(
        "    generic_indexed_access_unknown_fallback_count: {}",
        counters.generic_indexed_access_unknown_fallback_count
    );
    eprintln!(
        "    generic_indexed_access_invalid_key_count: {}",
        counters.generic_indexed_access_invalid_key_count
    );
    eprintln!(
        "    object_literal_property_check_count: {}",
        counters.object_literal_property_check_count
    );
    eprintln!(
        "    function_body_check_count: {}",
        counters.function_body_check_count
    );
    eprintln!(
        "    type_declaration_lookup_count: {}",
        counters.type_declaration_lookup_count
    );
    eprintln!(
        "    type_declaration_lookup_layer_steps_total: {}",
        counters.type_declaration_lookup_layer_steps_total
    );
    eprintln!("    type_clone_count: {}", counters.type_clone_count);
    eprintln!(
        "    object_type_clone_count: {}",
        counters.object_type_clone_count
    );
    eprintln!(
        "    object_type_id_copy_count: {}",
        counters.object_type_id_copy_count
    );
    eprintln!(
        "    union_type_clone_count: {}",
        counters.union_type_clone_count
    );
    eprintln!(
        "    symbol_name_clone_count: {}",
        counters.symbol_name_clone_count
    );
    eprintln!(
        "    string_key_clone_count: {}",
        counters.string_key_clone_count
    );
    eprintln!(
        "    flow_local_name_clone_count: {}",
        counters.flow_local_name_clone_count
    );
    eprintln!(
        "    string_path_lookup_count: {}",
        counters.string_path_lookup_count
    );
    eprintln!(
        "    canonical_file_id_lookup_count: {}",
        counters.canonical_file_id_lookup_count
    );
    eprintln!("    flow_function_count: {}", counters.flow_function_count);
    eprintln!(
        "    flow_function_skipped_count: {}",
        counters.flow_function_skipped_count
    );
    eprintln!(
        "    flow_statement_count: {}",
        counters.flow_statement_count
    );
    eprintln!(
        "    flow_expression_visit_count: {}",
        counters.flow_expression_visit_count
    );
    eprintln!(
        "    flow_identifier_read_count: {}",
        counters.flow_identifier_read_count
    );
    eprintln!(
        "    flow_scope_push_count: {}",
        counters.flow_scope_push_count
    );
    eprintln!(
        "    flow_scope_pop_count: {}",
        counters.flow_scope_pop_count
    );
    eprintln!(
        "    flow_future_declaration_collection_count: {}",
        counters.flow_future_declaration_collection_count
    );
    eprintln!(
        "    flow_future_declaration_entries_total: {}",
        counters.flow_future_declaration_entries_total
    );
    eprintln!(
        "    flow_state_clone_count: {}",
        counters.flow_state_clone_count
    );
    eprintln!(
        "    flow_scope_locals_clone_count: {}",
        counters.flow_scope_locals_clone_count
    );
    eprintln!(
        "    flow_state_full_clone_avoided_count: {}",
        counters.flow_state_full_clone_avoided_count
    );
    eprintln!(
        "    flow_branch_merge_count: {}",
        counters.flow_branch_merge_count
    );
    eprintln!(
        "    flow_branch_merge_scope_count: {}",
        counters.flow_branch_merge_scope_count
    );
    eprintln!(
        "    flow_branch_merge_local_iteration_count: {}",
        counters.flow_branch_merge_local_iteration_count
    );
    eprintln!(
        "    flow_branch_merge_fast_path_count: {}",
        counters.flow_branch_merge_fast_path_count
    );
    eprintln!(
        "    flow_branch_empty_delta_count: {}",
        counters.flow_branch_empty_delta_count
    );
    eprintln!(
        "    flow_branch_changed_local_count: {}",
        counters.flow_branch_changed_local_count
    );
    eprintln!(
        "    flow_read_lookup_count: {}",
        counters.flow_read_lookup_count
    );
    eprintln!(
        "    flow_read_lookup_scope_steps_total: {}",
        counters.flow_read_lookup_scope_steps_total
    );
    eprintln!(
        "    flow_return_analysis_walk_count: {}",
        counters.flow_return_analysis_walk_count
    );
    eprintln!(
        "    flow_truthiness_check_count: {}",
        counters.flow_truthiness_check_count
    );
    eprintln!(
        "    type_name_lookup_string_count: {}",
        counters.type_name_lookup_string_count
    );
    eprintln!(
        "    symbol_info_handle_copy_count: {}",
        counters.symbol_info_handle_copy_count
    );
    eprintln!(
        "    symbol_info_payload_deep_clone_count: {}",
        counters.symbol_info_payload_deep_clone_count
    );
    eprintln!(
        "    symbol_table_clone_count: {}",
        counters.symbol_table_clone_count
    );
    eprintln!(
        "    symbol_table_entry_handle_copy_count: {}",
        counters.symbol_table_entry_handle_copy_count
    );
    eprintln!(
        "    scope_stack_visible_rebuild_count: {}",
        counters.scope_stack_visible_rebuild_count
    );
    eprintln!(
        "    scope_stack_visible_symbol_handle_copy_count: {}",
        counters.scope_stack_visible_symbol_handle_copy_count
    );
    eprintln!(
        "    module_export_table_clone_count: {}",
        counters.module_export_table_clone_count
    );
    eprintln!(
        "    module_export_entry_clone_count: {}",
        counters.module_export_entry_clone_count
    );
    eprintln!(
        "    module_export_symbol_handle_copy_count: {}",
        counters.module_export_symbol_handle_copy_count
    );
    eprintln!(
        "    module_export_borrowed_lookup_count: {}",
        counters.module_export_borrowed_lookup_count
    );
    eprintln!(
        "    module_export_namespace_export_object_materialization_count: {}",
        counters.module_export_namespace_export_object_materialization_count
    );
    eprintln!(
        "    module_export_namespace_export_object_property_count: {}",
        counters.module_export_namespace_export_object_property_count
    );
    eprintln!(
        "    function_type_copy_from_expression_identifier_count: {}",
        counters.function_type_copy_from_expression_identifier_count
    );
    eprintln!(
        "    function_type_copy_from_expression_call_return_count: {}",
        counters.function_type_copy_from_expression_call_return_count
    );
    eprintln!(
        "    function_type_copy_from_expression_optional_call_return_count: {}",
        counters.function_type_copy_from_expression_optional_call_return_count
    );
    eprintln!(
        "    union_type_copy_from_expression_identifier_count: {}",
        counters.union_type_copy_from_expression_identifier_count
    );
    eprintln!(
        "    union_type_copy_from_expression_call_return_count: {}",
        counters.union_type_copy_from_expression_call_return_count
    );
    eprintln!(
        "    union_type_copy_from_expression_optional_call_return_count: {}",
        counters.union_type_copy_from_expression_optional_call_return_count
    );

    let function_type_counters = snapshot_function_type_counters();
    eprintln!(
        "    function_type_payload_alloc_count: {}",
        function_type_counters.function_type_payload_alloc_count
    );
    eprintln!(
        "    function_type_payload_deep_clone_count: {}",
        function_type_counters.function_type_payload_deep_clone_count
    );
    eprintln!(
        "    function_type_handle_copy_count: {}",
        function_type_counters.function_type_handle_copy_count
    );
    eprintln!(
        "    function_type_clone_count: {}",
        function_type_counters.function_type_clone_count
    );
    eprintln!(
        "    function_type_copy_from_expression_inference_count: {}",
        function_type_counters.function_type_copy_from_expression_inference_count
    );
    eprintln!(
        "    function_type_copy_from_call_resolution_count: {}",
        function_type_counters.function_type_copy_from_call_resolution_count
    );
    eprintln!(
        "    function_type_copy_from_property_call_resolution_count: {}",
        function_type_counters.function_type_copy_from_property_call_resolution_count
    );
    eprintln!(
        "    function_type_copy_from_function_body_setup_count: {}",
        function_type_counters.function_type_copy_from_function_body_setup_count
    );
    eprintln!(
        "    function_type_copy_from_return_checking_count: {}",
        function_type_counters.function_type_copy_from_return_checking_count
    );
    eprintln!(
        "    function_type_copy_from_expected_type_count: {}",
        function_type_counters.function_type_copy_from_expected_type_count
    );
    eprintln!(
        "    function_type_copy_from_symbol_table_count: {}",
        function_type_counters.function_type_copy_from_symbol_table_count
    );
    eprintln!(
        "    function_type_copy_from_module_export_count: {}",
        function_type_counters.function_type_copy_from_module_export_count
    );
    eprintln!(
        "    function_type_copy_from_scope_or_context_count: {}",
        function_type_counters.function_type_copy_from_scope_or_context_count
    );
    eprintln!(
        "    function_type_copy_from_substitution_unchanged_count: {}",
        function_type_counters.function_type_copy_from_substitution_unchanged_count
    );
    eprintln!(
        "    function_type_copy_from_substitution_changed_count: {}",
        function_type_counters.function_type_copy_from_substitution_changed_count
    );
    eprintln!(
        "    function_type_copy_from_diagnostic_formatting_count: {}",
        function_type_counters.function_type_copy_from_diagnostic_formatting_count
    );
    eprintln!(
        "    function_type_copy_unattributed_count: {}",
        function_type_counters.function_type_copy_unattributed_count
    );

    let union_type_counters = snapshot_union_type_counters();
    eprintln!(
        "    union_type_payload_alloc_count: {}",
        union_type_counters.union_type_payload_alloc_count
    );
    eprintln!(
        "    union_type_payload_deep_clone_count: {}",
        union_type_counters.union_type_payload_deep_clone_count
    );
    eprintln!(
        "    union_type_handle_copy_count: {}",
        union_type_counters.union_type_handle_copy_count
    );
    eprintln!(
        "    union_type_copy_from_expression_inference_count: {}",
        union_type_counters.union_type_copy_from_expression_inference_count
    );
    eprintln!(
        "    union_type_copy_from_call_resolution_count: {}",
        union_type_counters.union_type_copy_from_call_resolution_count
    );
    eprintln!(
        "    union_type_copy_from_property_call_resolution_count: {}",
        union_type_counters.union_type_copy_from_property_call_resolution_count
    );
    eprintln!(
        "    union_type_copy_from_function_body_setup_count: {}",
        union_type_counters.union_type_copy_from_function_body_setup_count
    );
    eprintln!(
        "    union_type_copy_from_return_checking_count: {}",
        union_type_counters.union_type_copy_from_return_checking_count
    );
    eprintln!(
        "    union_type_copy_from_expected_type_count: {}",
        union_type_counters.union_type_copy_from_expected_type_count
    );
    eprintln!(
        "    union_type_copy_from_symbol_table_count: {}",
        union_type_counters.union_type_copy_from_symbol_table_count
    );
    eprintln!(
        "    union_type_copy_from_module_export_count: {}",
        union_type_counters.union_type_copy_from_module_export_count
    );
    eprintln!(
        "    union_type_copy_from_scope_or_context_count: {}",
        union_type_counters.union_type_copy_from_scope_or_context_count
    );
    eprintln!(
        "    union_type_copy_from_substitution_unchanged_count: {}",
        union_type_counters.union_type_copy_from_substitution_unchanged_count
    );
    eprintln!(
        "    union_type_copy_from_substitution_changed_count: {}",
        union_type_counters.union_type_copy_from_substitution_changed_count
    );
    eprintln!(
        "    union_type_copy_from_diagnostic_formatting_count: {}",
        union_type_counters.union_type_copy_from_diagnostic_formatting_count
    );
    eprintln!(
        "    union_type_copy_unattributed_count: {}",
        union_type_counters.union_type_copy_unattributed_count
    );
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum TableCloneKind {
    General,
    GeneratedDefaultLib,
    DependencyDeclaration,
}

#[derive(Debug, Clone, Copy)]
enum TableMergeKind {
    General,
}

fn clone_type_declaration_table(
    table: &TypeDeclarationTable,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    kind: TableCloneKind,
) -> TypeDeclarationTable {
    record_type_declaration_table_clone(timings, table.len(), kind);
    table.clone()
}

pub(crate) fn record_string_path_lookup() {
    record_program_counter(|c| c.string_path_lookup_count += 1);
}

pub(crate) fn record_canonical_file_id_lookup() {
    record_program_counter(|c| c.canonical_file_id_lookup_count += 1);
}

pub(crate) fn record_declaration_lookup(layer_count: usize) {
    record_program_counter(|c| {
        c.declaration_lookup_count += 1;
        c.declaration_lookup_layer_count_total += layer_count as u64;
    });
}

pub(crate) fn record_expression_check() {
    record_program_counter(|c| c.expression_check_count += 1);
}

pub(crate) fn record_expression_infer() {
    record_program_counter(|c| c.expression_infer_count += 1);
}

pub(crate) fn record_assignability_check() {
    record_program_counter(|c| c.assignability_check_count += 1);
}

pub(crate) fn record_property_lookup() {
    record_program_counter(|c| c.property_lookup_count += 1);
}

pub(crate) fn record_call_resolution() {
    record_program_counter(|c| c.call_resolution_count += 1);
}

pub(crate) fn record_generic_call_inference_attempt() {
    record_program_counter(|c| c.generic_call_inference_attempt_count += 1);
}

pub(crate) fn record_generic_call_inference_success() {
    record_program_counter(|c| c.generic_call_inference_success_count += 1);
}

pub(crate) fn record_generic_call_inference_failed() {
    record_program_counter(|c| c.generic_call_inference_failed_count += 1);
}

pub(crate) fn record_generic_call_inference_explicit_type_args_skip() {
    record_program_counter(|c| c.generic_call_inference_explicit_type_args_skip_count += 1);
}

pub(crate) fn record_generic_call_inference_unresolved_argument_skip() {
    record_program_counter(|c| c.generic_call_inference_unresolved_argument_skip_count += 1);
}

pub(crate) fn record_generic_call_inference_tuple_return_suppressed() {
    record_program_counter(|c| c.generic_call_inference_tuple_return_suppressed_count += 1);
}

pub(crate) fn record_generic_call_inference_candidate() {
    record_program_counter(|c| c.generic_call_inference_candidate_count += 1);
}

pub(crate) fn record_generic_indexed_access_attempt() {
    record_program_counter(|c| c.generic_indexed_access_attempt_count += 1);
}

pub(crate) fn record_generic_indexed_access_substituted_receiver() {
    record_program_counter(|c| c.generic_indexed_access_substituted_receiver_count += 1);
}

pub(crate) fn record_generic_indexed_access_substituted_key() {
    record_program_counter(|c| c.generic_indexed_access_substituted_key_count += 1);
}

pub(crate) fn record_generic_indexed_access_success() {
    record_program_counter(|c| c.generic_indexed_access_success_count += 1);
}

pub(crate) fn record_generic_indexed_access_unknown_fallback() {
    record_program_counter(|c| c.generic_indexed_access_unknown_fallback_count += 1);
}

pub(crate) fn record_generic_indexed_access_invalid_key() {
    record_program_counter(|c| c.generic_indexed_access_invalid_key_count += 1);
}

pub(crate) fn record_object_literal_property_check() {
    record_program_counter(|c| c.object_literal_property_check_count += 1);
}

pub(crate) fn record_function_body_check() {
    record_program_counter(|c| c.function_body_check_count += 1);
}

pub(crate) fn record_type_declaration_lookup(layer_steps: usize) {
    record_program_counter(|c| {
        c.type_declaration_lookup_count += 1;
        c.type_declaration_lookup_layer_steps_total += layer_steps as u64;
    });
}

pub(crate) fn record_module_scope_cache_hit() {
    record_program_counter(|c| c.module_scope_cache_hits += 1);
}

pub(crate) fn record_module_scope_cache_miss() {
    record_program_counter(|c| c.module_scope_cache_misses += 1);
}

pub(crate) fn record_type_clone_count() {
    record_program_counter(|c| c.type_clone_count += 1);
}

pub(crate) fn record_checker_arena_alloc_count() {
    record_program_counter(|c| c.checker_arena_alloc_count += 1);
}

pub(crate) fn record_arena_declaration_key_alloc_count() {
    record_program_counter(|c| c.arena_declaration_key_alloc_count += 1);
}

pub(crate) fn record_arena_type_declaration_payload_alloc_count() {
    record_program_counter(|c| c.arena_type_declaration_payload_alloc_count += 1);
}

pub(crate) fn record_arena_object_type_payload_alloc_count() {
    record_program_counter(|c| c.arena_object_type_payload_alloc_count += 1);
}

pub(crate) fn record_type_declaration_payload_deep_clone_count() {
    record_program_counter(|c| c.type_declaration_payload_deep_clone_count += 1);
}

#[allow(dead_code)]
pub(crate) fn record_object_type_payload_deep_clone_count() {
    record_program_counter(|c| c.object_type_payload_deep_clone_count += 1);
}

pub(crate) fn record_object_type_clone_count() {
    record_program_counter(|c| c.object_type_clone_count += 1);
}

pub(crate) fn record_object_type_id_copy_count() {
    record_program_counter(|c| c.object_type_id_copy_count += 1);
}

pub(crate) fn record_union_type_clone_count() {
    record_program_counter(|c| c.union_type_clone_count += 1);
}

#[allow(dead_code)]
pub(crate) fn record_symbol_name_clone_count(count: usize) {
    record_program_counter(|c| c.symbol_name_clone_count += count as u64);
}

#[allow(dead_code)]
pub(crate) fn record_string_key_clone_count(count: usize) {
    record_program_counter(|c| c.string_key_clone_count += count as u64);
}

#[allow(dead_code)]
pub(crate) fn record_flow_local_name_clone_count(count: usize) {
    record_program_counter(|c| c.flow_local_name_clone_count += count as u64);
}

pub(crate) fn record_flow_function_count() {
    record_program_counter(|c| c.flow_function_count += 1);
}

pub(crate) fn record_flow_function_skipped_count() {
    record_program_counter(|c| c.flow_function_skipped_count += 1);
}

pub(crate) fn record_flow_statement_count() {
    record_program_counter(|c| c.flow_statement_count += 1);
}

pub(crate) fn record_flow_expression_visit_count() {
    record_program_counter(|c| c.flow_expression_visit_count += 1);
}

pub(crate) fn record_flow_identifier_read_count() {
    record_program_counter(|c| c.flow_identifier_read_count += 1);
}

pub(crate) fn record_flow_scope_push_count() {
    record_program_counter(|c| c.flow_scope_push_count += 1);
}

pub(crate) fn record_flow_scope_pop_count() {
    record_program_counter(|c| c.flow_scope_pop_count += 1);
}

pub(crate) fn record_flow_future_declaration_collection_count(entry_count: usize) {
    record_program_counter(|c| {
        c.flow_future_declaration_collection_count += 1;
        c.flow_future_declaration_entries_total += entry_count as u64;
    });
}

pub(crate) fn record_flow_state_clone_count(local_count: usize) {
    record_program_counter(|c| {
        c.flow_state_clone_count += 1;
        c.flow_scope_locals_clone_count += local_count as u64;
    });
}

pub(crate) fn record_flow_state_full_clone_avoided_count() {
    record_program_counter(|c| c.flow_state_full_clone_avoided_count += 1);
}

pub(crate) fn record_flow_branch_merge_count(scope_count: usize) {
    record_program_counter(|c| {
        c.flow_branch_merge_count += 1;
        c.flow_branch_merge_scope_count += scope_count as u64;
    });
}

pub(crate) fn record_flow_branch_merge_local_iteration_count(count: usize) {
    record_program_counter(|c| {
        c.flow_branch_merge_local_iteration_count += count as u64;
    });
}

pub(crate) fn record_flow_branch_merge_fast_path_count() {
    record_program_counter(|c| c.flow_branch_merge_fast_path_count += 1);
}

pub(crate) fn record_flow_branch_empty_delta_count() {
    record_program_counter(|c| c.flow_branch_empty_delta_count += 1);
}

pub(crate) fn record_flow_branch_changed_local_count(count: usize) {
    record_program_counter(|c| {
        c.flow_branch_changed_local_count += count as u64;
    });
}

pub(crate) fn record_flow_read_lookup_count(scope_steps: usize) {
    record_program_counter(|c| {
        c.flow_read_lookup_count += 1;
        c.flow_read_lookup_scope_steps_total += scope_steps as u64;
    });
}

pub(crate) fn record_flow_return_analysis_walk_count() {
    record_program_counter(|c| c.flow_return_analysis_walk_count += 1);
}

pub(crate) fn record_flow_truthiness_check_count() {
    record_program_counter(|c| c.flow_truthiness_check_count += 1);
}

pub(crate) fn record_type_name_lookup_string_count(count: usize) {
    record_program_counter(|c| c.type_name_lookup_string_count += count as u64);
}

pub(crate) fn record_symbol_info_handle_copy_count(count: u64) {
    record_program_counter(|c| c.symbol_info_handle_copy_count += count);
}

pub(crate) fn record_symbol_info_payload_deep_clone_count() {
    record_program_counter(|c| c.symbol_info_payload_deep_clone_count += 1);
}

pub(crate) fn record_symbol_table_clone_count() {
    record_program_counter(|c| c.symbol_table_clone_count += 1);
}

pub(crate) fn record_symbol_table_entry_handle_copy_count(count: u64) {
    record_program_counter(|c| c.symbol_table_entry_handle_copy_count += count);
}

#[allow(dead_code)]
pub(crate) fn record_scope_stack_visible_rebuild_count() {
    record_program_counter(|c| c.scope_stack_visible_rebuild_count += 1);
}

pub(crate) fn record_scope_stack_visible_symbol_handle_copy_count(count: u64) {
    record_program_counter(|c| c.scope_stack_visible_symbol_handle_copy_count += count);
}

pub(crate) fn record_function_type_copy_from_expression_identifier_count() {
    record_program_counter(|c| c.function_type_copy_from_expression_identifier_count += 1);
}

pub(crate) fn record_function_type_copy_from_expression_call_return_count() {
    record_program_counter(|c| c.function_type_copy_from_expression_call_return_count += 1);
}

pub(crate) fn record_function_type_copy_from_expression_optional_call_return_count() {
    record_program_counter(|c| {
        c.function_type_copy_from_expression_optional_call_return_count += 1
    });
}

pub(crate) fn record_union_type_copy_from_expression_identifier_count() {
    record_program_counter(|c| c.union_type_copy_from_expression_identifier_count += 1);
}

pub(crate) fn record_union_type_copy_from_expression_call_return_count() {
    record_program_counter(|c| c.union_type_copy_from_expression_call_return_count += 1);
}

pub(crate) fn record_union_type_copy_from_expression_optional_call_return_count() {
    record_program_counter(|c| c.union_type_copy_from_expression_optional_call_return_count += 1);
}

fn record_type_declaration_table_clone(
    _timings: Option<&Arc<Mutex<ProgramTimings>>>,
    entry_count: usize,
    kind: TableCloneKind,
) {
    record_program_counter(|c| {
        c.type_declaration_table_clone_count += 1;
        c.type_declaration_id_copy_count += entry_count as u64;
        match kind {
            TableCloneKind::General => {}
            TableCloneKind::GeneratedDefaultLib => {
                c.generated_default_lib_table_clone_count += 1;
            }
            TableCloneKind::DependencyDeclaration => {
                c.dependency_declaration_table_clone_count += 1;
            }
        }
    });
}

fn record_type_declaration_table_merge(
    _timings: Option<&Arc<Mutex<ProgramTimings>>>,
    entry_count: usize,
    _kind: TableMergeKind,
) {
    record_program_counter(|c| {
        c.type_declaration_table_merge_count += 1;
        c.type_declaration_entries_merged_total += entry_count as u64;
    });
}

fn record_program_counter(update: impl FnOnce(&mut ProgramCounters)) {
    let lock = PROGRAM_COUNTERS.get_or_init(|| Mutex::new(ProgramCounters::default()));
    if let Ok(mut guard) = lock.lock() {
        update(&mut guard);
    }
}

pub(crate) fn record_module_export_table_clone_count() {
    record_program_counter(|c| c.module_export_table_clone_count += 1);
}

pub(crate) fn record_module_export_entry_clone_count(count: u64) {
    record_program_counter(|c| c.module_export_entry_clone_count += count);
}

pub(crate) fn record_module_export_symbol_handle_copy_count(count: u64) {
    record_program_counter(|c| c.module_export_symbol_handle_copy_count += count);
}

pub(crate) fn record_module_export_borrowed_lookup_count() {
    record_program_counter(|c| c.module_export_borrowed_lookup_count += 1);
}

pub(crate) fn record_module_export_namespace_export_object_materialization_count() {
    record_program_counter(|c| c.module_export_namespace_export_object_materialization_count += 1);
}

pub(crate) fn record_module_export_namespace_export_object_property_count(count: u64) {
    record_program_counter(|c| c.module_export_namespace_export_object_property_count += count);
}

fn reset_program_counters() {
    let lock = PROGRAM_COUNTERS.get_or_init(|| Mutex::new(ProgramCounters::default()));
    if let Ok(mut guard) = lock.lock() {
        *guard = ProgramCounters::default();
    }
}

fn snapshot_program_counters() -> ProgramCounters {
    let lock = PROGRAM_COUNTERS.get_or_init(|| Mutex::new(ProgramCounters::default()));
    lock.lock().map(|guard| guard.clone()).unwrap_or_default()
}
