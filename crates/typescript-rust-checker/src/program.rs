use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedFunctionDeclaration,
    ParsedImportKind, ParsedStatement, TextSpan, parse_source,
};
use typescript_rust_types::FunctionType;

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::{CheckerContext, CheckerOptions, CompatibilityStats, FileKind};
use crate::driver::validate_direct_utility_aliases;
use crate::driver::{collect_type_declarations, validate_local_type_declarations};
use crate::modules::{
    ModuleExportTable, ModuleImportBindings, build_module_export_table,
    resolve_module_export_tables, resolve_module_imports,
};
use crate::symbols::{SymbolTable, TypeDeclarationTable};

#[derive(Debug, Clone)]
pub struct SourceFileInput {
    pub file_name: String,
    pub source_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FunctionDeclarationLocation {
    file_index: usize,
    statement_index: usize,
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
    module_resolution_scopes: Vec<Option<Arc<TypeDeclarationTable>>>,
}

#[derive(Debug)]
struct FileCheckResult {
    file_index: usize,
    diagnostics: Vec<Diagnostic>,
    stats: CompatibilityStats,
}

#[derive(Debug, Clone)]
struct ModuleAnalysis {
    local_type_declarations: TypeDeclarationTable,
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

#[derive(Debug, Default)]
struct ProgramTimings {
    parsing: Duration,
    type_declaration_collection: Duration,
    module_binding: Duration,
    declaration_validation: Duration,
    per_file_statement_checking: Duration,
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

    let timings_enabled = std::env::var_os("TYPESCRIPT_RUST_TIMINGS").is_some();
    let timings = timings_enabled.then(|| Arc::new(Mutex::new(ProgramTimings::default())));

    let parse_start = Instant::now();
    let parsed_files = parse_program_files(files);
    record_program_timing(timings.as_ref(), |timings| {
        timings.parsing += parse_start.elapsed()
    });
    let file_kinds = parsed_files
        .iter()
        .map(|file| (file.file_name.clone(), file.file_kind))
        .collect::<HashMap<_, _>>();
    let first_file_name = parsed_files
        .first()
        .map(|file| file.file_name.clone())
        .unwrap_or_default();
    let mut ctx = CheckerContext::new(first_file_name, options, file_kinds);

    crate::builtins::inject_builtins(&mut ctx);

    let mut global_symbols = SymbolTable::new();
    let mut function_signatures = HashMap::new();

    emit_parser_diagnostics(&parsed_files, &mut ctx);
    collect_ambient_globals(&parsed_files, &mut ctx);
    collect_ambient_modules(&parsed_files, &mut ctx);

    let type_collection_start = Instant::now();
    collect_global_type_declarations(&parsed_files, &mut ctx);
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_collection_start.elapsed()
    });
    let global_type_declarations = ctx.type_declarations.clone();
    collect_global_function_signatures(
        &parsed_files,
        &mut global_symbols,
        &mut function_signatures,
        &mut ctx,
    );
    collect_global_variables(&parsed_files, &mut global_symbols, &mut ctx);

    // PRELIMINARY PASS: collect types and resolve imports/exports to make them available for function signature collection
    let type_collection_start = Instant::now();
    let (local_type_declarations_by_module, preliminary_module_import_bindings) =
        collect_preliminary_module_type_bindings(&parsed_files, &mut ctx);
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_collection_start.elapsed()
    });

    let type_collection_start = Instant::now();
    let preliminary_module_analyses = collect_module_analyses_with_bindings(
        &parsed_files,
        &local_type_declarations_by_module,
        &preliminary_module_import_bindings,
        &mut ctx,
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_collection_start.elapsed()
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
    let module_export_tables =
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, &mut ctx);
    let preliminary_module_resolution_scopes = build_module_resolution_scopes(
        &local_type_declarations_by_module,
        &preliminary_module_import_bindings,
    );
    let module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &preliminary_module_analyses,
        &module_export_tables,
        &preliminary_module_resolution_scopes,
        &mut ctx,
    );
    let module_resolution_scopes =
        build_module_resolution_scopes(&local_type_declarations_by_module, &module_import_bindings);
    let diagnostics_before_second_bindings = ctx.diagnostics().len();
    let module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &preliminary_module_analyses,
        &module_export_tables,
        &module_resolution_scopes,
        &mut ctx,
    );
    ctx.truncate_diagnostics(diagnostics_before_second_bindings);
    let module_resolution_scopes =
        build_module_resolution_scopes(&local_type_declarations_by_module, &module_import_bindings);
    let type_collection_start = Instant::now();
    let module_analyses = collect_module_analyses_with_bindings(
        &parsed_files,
        &local_type_declarations_by_module,
        &module_import_bindings,
        &mut ctx,
    );
    record_program_timing(timings.as_ref(), |timings| {
        timings.type_declaration_collection += type_collection_start.elapsed()
    });
    let local_module_export_tables = module_analyses
        .iter()
        .map(|analysis| {
            analysis
                .as_ref()
                .map(|analysis| analysis.local_export_table.clone())
        })
        .collect::<Vec<_>>();
    let module_export_tables =
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, &mut ctx);
    let diagnostics_before_final_bindings = ctx.diagnostics().len();
    let module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &module_analyses,
        &module_export_tables,
        &module_resolution_scopes,
        &mut ctx,
    );
    ctx.truncate_diagnostics(diagnostics_before_final_bindings);
    let module_resolution_scopes =
        build_module_resolution_scopes(&local_type_declarations_by_module, &module_import_bindings);
    record_program_timing(timings.as_ref(), |timings| {
        timings.module_binding += module_binding_start.elapsed()
    });
    let shared_state = ProgramCheckSharedState {
        global_type_declarations,
        global_symbols,
        function_signatures,
        module_analyses,
        module_import_bindings,
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
) -> (
    Vec<Option<TypeDeclarationTable>>,
    Vec<Option<ModuleImportBindings>>,
) {
    let mut local_type_declarations_by_module = Vec::with_capacity(parsed_files.len());
    let mut preliminary_local_export_tables = Vec::with_capacity(parsed_files.len());

    let initial_diagnostics_len = ctx.diagnostics().len();

    for parsed_file in parsed_files {
        if !parsed_file.is_module {
            local_type_declarations_by_module.push(None);
            preliminary_local_export_tables.push(None);
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());

        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());

        collect_type_declarations(&parsed_file.statements, ctx);
        let local_type_declarations = ctx.type_declarations.clone();

        let preliminary_export_table = build_module_export_table(
            parsed_file,
            &local_type_declarations,
            &SymbolTable::new(), // empty symbols
            None,
            ctx,
        );

        ctx.type_declarations = saved_type_declarations;
        ctx.symbols = saved_symbols;

        local_type_declarations_by_module.push(Some(local_type_declarations));
        preliminary_local_export_tables.push(Some(preliminary_export_table));
    }

    let preliminary_module_export_tables =
        resolve_module_export_tables(parsed_files, &preliminary_local_export_tables, ctx);

    let preliminary_module_resolution_scopes = local_type_declarations_by_module
        .iter()
        .map(|td| td.as_ref().map(|td| Arc::new(td.clone())))
        .collect::<Vec<_>>();

    let mut preliminary_module_import_bindings = Vec::with_capacity(parsed_files.len());
    for (_file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module {
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

    // Discard any diagnostics emitted during the preliminary pass
    ctx.truncate_diagnostics(initial_diagnostics_len);

    (
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
    )
}

fn collect_module_analyses_with_bindings(
    parsed_files: &[ParsedProgramFile],
    _local_type_declarations_by_module: &[Option<TypeDeclarationTable>],
    preliminary_module_import_bindings: &[Option<ModuleImportBindings>],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleAnalysis>> {
    let mut analyses = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module {
            analyses.push(None);
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());

        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());

        // Re-run collect_type_declarations to emit the correct TS2300 diagnostics
        collect_type_declarations(&parsed_file.statements, ctx);
        let local_type_declarations = ctx.type_declarations.clone();

        // Set up the full type environment for function signature collection
        let mut full_type_declarations = ctx.ambient_global_type_declarations.clone();
        for (k, v) in local_type_declarations.iter() {
            let _ = full_type_declarations.insert(k.clone(), v.clone());
        }
        if let Some(imported) = &preliminary_module_import_bindings[file_index] {
            for (k, v) in imported.type_declarations.iter() {
                let _ = full_type_declarations.insert(k.clone(), v.clone());
            }
        }
        let full_type_declarations_scope = Arc::new(full_type_declarations.clone());
        ctx.type_declarations = full_type_declarations;

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

        let export_table = build_module_export_table(
            parsed_file,
            &local_type_declarations,
            &local_symbols,
            Some(full_type_declarations_scope),
            ctx,
        );

        ctx.type_declarations = saved_type_declarations;
        ctx.symbols = saved_symbols;

        analyses.push(Some(ModuleAnalysis {
            local_type_declarations,
            local_symbols,
            local_function_signatures,
            local_export_table: export_table,
        }));
    }

    analyses
}

fn parse_program_files(files: Vec<SourceFileInput>) -> Vec<ParsedProgramFile> {
    files
        .into_iter()
        .map(|input| {
            let parsed = parse_source(&input.source_text, &input.file_name);
            let file_name = parsed.file_name;
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
    local_type_declarations_by_module: &[Option<TypeDeclarationTable>],
    module_import_bindings: &[Option<ModuleImportBindings>],
) -> Vec<Option<Arc<TypeDeclarationTable>>> {
    local_type_declarations_by_module
        .iter()
        .enumerate()
        .map(|(file_index, local_type_declarations)| {
            let Some(local_type_declarations) = local_type_declarations else {
                return None;
            };

            let mut merged_type_declarations = local_type_declarations.clone();
            if let Some(imported) = module_import_bindings
                .get(file_index)
                .and_then(|bindings| bindings.as_ref())
            {
                for (name, declaration) in imported.type_declarations.iter() {
                    let _ = merged_type_declarations.insert(name.clone(), declaration.clone());
                }
            }

            Some(Arc::new(merged_type_declarations))
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
        || lower.contains("/generated/")
        || lower.contains("/dist/")
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

fn collect_global_type_declarations(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        if parsed_file.is_module && !parsed_file.file_kind.is_declaration() {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        collect_type_declarations(&parsed_file.statements, ctx);
    }
}

fn collect_global_function_signatures(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
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

        let imported_bindings = shared_state.module_import_bindings[file_index]
            .clone()
            .unwrap_or_default();

        let mut merged_type_declarations = ctx.ambient_global_type_declarations.clone();
        if let Some(module_resolution_scope) =
            shared_state.module_resolution_scopes[file_index].as_ref()
        {
            for (name, declaration) in module_resolution_scope.iter() {
                let _ = merged_type_declarations.insert(name.clone(), declaration.clone());
            }
        } else {
            for (name, declaration) in module_analysis.local_type_declarations.iter() {
                let _ = merged_type_declarations.insert(name.clone(), declaration.clone());
            }
        }
        for (name, declaration) in imported_bindings.type_declarations.iter() {
            let _ = merged_type_declarations.insert(name.clone(), declaration.clone());
        }

        let mut merged_symbols = ctx.ambient_global_symbols.clone();
        for (name, symbol) in module_analysis.local_symbols.iter() {
            let _ = merged_symbols.insert(name.clone(), symbol.clone());
        }
        for (name, symbol) in imported_bindings.symbols.iter() {
            if merged_symbols.get(name).is_none() {
                merged_symbols.insert(name.clone(), symbol.clone());
            }
        }

        ctx.type_declarations = merged_type_declarations;
        ctx.set_symbols(merged_symbols.clone());

        let validation_start = Instant::now();
        validate_local_type_declarations(&parsed_file.statements, &parsed_file.file_name, ctx);
        validate_direct_utility_aliases(&parsed_file.statements, ctx);
        record_program_timing(timings, |timings| {
            timings.declaration_validation += validation_start.elapsed()
        });

        let mut signature_ctx = ctx.clone();
        signature_ctx.diagnostics.clear();
        signature_ctx.utility_diagnostic_keys.clear();
        signature_ctx.resolved_named_types =
            std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        let mut signature_local_symbols = crate::symbols::SymbolTable::new();
        for (name, symbol) in merged_symbols.iter() {
            if !matches!(symbol.kind, crate::symbols::SymbolKind::Function) {
                signature_local_symbols.insert(name.clone(), symbol.clone());
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
        let mut script_td = shared_state.global_type_declarations.clone();
        for (name, declaration) in ctx.ambient_global_type_declarations.iter() {
            let _ = script_td.insert(name.clone(), declaration.clone());
        }
        ctx.type_declarations = script_td;

        let mut script_sym = shared_state.global_symbols.clone();
        for (name, symbol) in ctx.ambient_global_symbols.iter() {
            let _ = script_sym.insert(name.clone(), symbol.clone());
        }
        ctx.set_symbols(script_sym);

        let validation_start = Instant::now();
        validate_local_type_declarations(&parsed_file.statements, &parsed_file.file_name, ctx);
        validate_direct_utility_aliases(&parsed_file.statements, ctx);
        record_program_timing(timings, |timings| {
            timings.declaration_validation += validation_start.elapsed()
        });

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
    module_resolution_scopes: &[Option<Arc<TypeDeclarationTable>>],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleImportBindings>> {
    let mut module_import_bindings = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module {
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

fn collect_function_signatures_from_statements(
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
            var::check_variable_declaration(variable, ctx);
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
    let body_root_symbols = saved_symbols.clone();
    ctx.symbols = body_root_symbols;
    let Some(function_type) = function_signatures.get(&declaration_location).cloned() else {
        check_function::check_function_declaration(function, ctx);
        ctx.symbols = saved_symbols;
        return;
    };

    let type_parameters = function.type_parameters.clone();
    check_function::check_function_declaration_body(
        function,
        &function_type,
        &type_parameters,
        ctx,
    );
    ctx.symbols = saved_symbols;
}

fn collect_ambient_globals(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        if !parsed_file.file_kind.is_declaration() {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());

        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        collect_type_declarations(&parsed_file.statements, ctx);
        let ambient_td = ctx.type_declarations.clone();

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
    }
}

fn collect_ambient_modules(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    let mut ambient_module_entries = Vec::<AmbientModuleEntry>::new();
    let mut ambient_module_indexes = HashMap::<String, usize>::new();

    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());
        for statement in &parsed_file.statements {
            let ParsedStatement::DeclareModuleDeclaration(module) = statement else {
                continue;
            };

            let saved_type_declarations =
                std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
            let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());

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
            let raw_export_table = build_module_export_table(
                &temp_file,
                &current_type_declarations,
                &current_symbols,
                Some(Arc::new(current_type_declarations.clone())),
                ctx,
            );
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
        }
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
}

fn merge_module_export_tables(target: &mut ModuleExportTable, source: &ModuleExportTable) {
    for (name, declaration) in source.type_declarations.iter() {
        if target.type_declarations.get(name).is_none() {
            let _ = target
                .type_declarations
                .insert(name.clone(), declaration.clone());
        }
    }

    for (name, symbol) in source.symbols.iter() {
        if target.symbols.get(name).is_none() {
            let _ = target.symbols.insert(name.clone(), symbol.clone());
        }
    }

    if target.default_symbol.is_none() {
        target.default_symbol = source.default_symbol.clone();
    }

    target.has_unresolved_star_export |= source.has_unresolved_star_export;
    target.has_incomplete_declaration_surface |= source.has_incomplete_declaration_surface;
}

fn collect_global_variables(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    for parsed_file in parsed_files {
        if parsed_file.is_module && !parsed_file.file_kind.is_declaration() {
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
                                ty: ty,
                                function_signature: None,
                            },
                        );
                    }
                }
            }
        }
    }
}

fn record_program_timing(
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

fn render_program_timings(timings: &Arc<Mutex<ProgramTimings>>) {
    let Ok(timings) = timings.lock() else {
        return;
    };

    eprintln!("Timings:");
    eprintln!("  parsing: {}", format_duration(timings.parsing));
    eprintln!(
        "  type_declaration_collection: {}",
        format_duration(timings.type_declaration_collection)
    );
    eprintln!(
        "  module_binding: {}",
        format_duration(timings.module_binding)
    );
    eprintln!(
        "  declaration_validation: {}",
        format_duration(timings.declaration_validation)
    );
    eprintln!(
        "  per_file_statement_checking: {}",
        format_duration(timings.per_file_statement_checking)
    );
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}
