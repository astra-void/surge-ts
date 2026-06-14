//! Module type/import binding collection across the multi-pass fixpoint.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use typescript_rust_diagnostics::Diagnostic;

use super::*;

use crate::context::{CheckerContext, FileKind};
use crate::driver::{collect_global_augmentations_from_statements, collect_type_declarations};
use crate::modules::{
    ModuleExportTable, ModuleImportBindings, build_module_export_table,
    resolve_module_export_tables, resolve_module_imports,
};
use crate::symbols::{SymbolTable, TypeDeclarationScope, TypeDeclarationTable};

pub(crate) fn collect_preliminary_module_type_bindings(
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

pub(crate) fn collect_module_analyses_with_bindings(
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

pub(crate) fn build_module_resolution_scopes(
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

pub(crate) fn collect_module_import_bindings(
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

pub(crate) fn should_replay_preliminary_diagnostic(
    diagnostic: &Diagnostic,
    ctx: &CheckerContext,
) -> bool {
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

pub(crate) fn merge_module_import_bindings(
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
