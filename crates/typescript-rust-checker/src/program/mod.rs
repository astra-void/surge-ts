use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{ParsedStatement, parse_source};
use typescript_rust_types::FunctionType;

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.
pub(crate) use crate::metrics::*;

use crate::context::{CheckerContext, CheckerOptions, CompatibilityStats, FileKind};
use crate::driver::validate_direct_utility_aliases;
use crate::driver::{sync_global_this_symbol, validate_local_type_declarations};
use crate::load_generated_default_lib_inputs;
use crate::modules::{ModuleExportTable, ModuleImportBindings, resolve_module_export_tables};
use crate::paths::canonicalize_if_exists_string;
use crate::symbols::{
    SymbolTable, TypeDeclarationScope, TypeDeclarationTable, clone_symbol_info_handle,
};

mod ambient;
mod binding;
mod classes;
mod globals;
mod statements;

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
pub(crate) struct ModuleAnalysis {
    local_type_declarations: Arc<TypeDeclarationTable>,
    local_symbols: SymbolTable,
    #[allow(dead_code)]
    local_function_signatures: HashMap<FunctionDeclarationLocation, FunctionType>,
    local_export_table: ModuleExportTable,
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
    set_counters_enabled(timings_enabled);
    let timings = timings_enabled.then(|| Arc::new(Mutex::new(ProgramTimings::default())));
    reset_program_counters();
    crate::paths::clear_canonicalize_cache();
    crate::modules::clear_relative_module_cache();

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
                source_text: input.source_text,
                statements: parsed.statements,
                parser_errors: parsed.parser_errors,
                is_module: parsed.is_module,
                file_kind: classify_file_kind(&file_name),
            }
        })
        .collect()
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

fn clone_type_declaration_table(
    table: &TypeDeclarationTable,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
    kind: TableCloneKind,
) -> TypeDeclarationTable {
    record_type_declaration_table_clone(timings, table.len(), kind);
    table.clone()
}
