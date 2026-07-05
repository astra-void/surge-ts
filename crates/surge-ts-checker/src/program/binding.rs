//! Module type/import binding collection across the multi-pass fixpoint.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use surge_ts_diagnostics::Diagnostic;

use super::*;

use crate::context::{CheckerContext, FileKind};
use crate::driver::{collect_type_declarations, lower_global_augmentation_values_from_statements};
use crate::modules::{
    ModuleExportTable, ModuleImportBindings, build_module_export_table,
    resolve_module_export_tables, resolve_module_imports,
};
use crate::symbols::{
    SymbolTable, TypeDeclarationInfo, TypeDeclarationScope, TypeDeclarationTable,
};

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
        let raw_local_type_declarations = Arc::new(std::mem::take(&mut ctx.type_declarations));
        let preliminary_raw_scope = Arc::new(TypeDeclarationScope::new(vec![
            raw_local_type_declarations.clone(),
        ]));
        let local_type_declarations = Arc::new(attach_resolution_scope_to_declarations(
            raw_local_type_declarations.as_ref(),
            preliminary_raw_scope,
        ));
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

        let preliminary_resolution_scope = Arc::new(TypeDeclarationScope::new(vec![
            local_type_declarations.clone(),
        ]));
        let preliminary_export_table = build_module_export_table(
            parsed_file,
            local_type_declarations.as_ref(),
            &SymbolTable::new(), // empty symbols
            &SymbolTable::new(), // imports not resolved yet in the preliminary pass
            Some(preliminary_resolution_scope),
            ctx,
        );

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

fn attach_resolution_scope_to_declarations(
    declarations: &TypeDeclarationTable,
    scope: Arc<TypeDeclarationScope>,
) -> TypeDeclarationTable {
    let mut attached = TypeDeclarationTable::new();
    for (name, declaration) in declarations.iter() {
        let declaration = match declaration.clone() {
            TypeDeclarationInfo::Alias(mut alias) => {
                alias.resolution_scope = Some(scope.clone());
                TypeDeclarationInfo::Alias(alias)
            }
            TypeDeclarationInfo::Interface(mut interface) => {
                interface.resolution_scope = Some(scope.clone());
                TypeDeclarationInfo::Interface(interface)
            }
        };
        let _ = attached.insert(name.as_ref(), declaration);
    }
    attached
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
        let imported_layers = preliminary_module_import_bindings[file_index]
            .as_ref()
            .map(|bindings| bindings.scope_layers());
        let mut scope_layers = vec![local_type_declarations.clone()];
        if let Some(imported_layers) = imported_layers {
            scope_layers.extend(imported_layers);
        }
        let full_type_declarations_scope = Arc::new(TypeDeclarationScope::new(scope_layers));
        ctx.type_declarations = local_type_declarations.as_ref().clone();
        ctx.type_declaration_scope = Some(full_type_declarations_scope.clone());
        record_program_timing(timings, |timings| {
            timings.declaration_table_merging_cloning += merge_start.elapsed()
        });

        // `typeof <value>` inside a parameter annotation resolves against the
        // module's value bindings — imports and `const`s alike — which signature
        // collection alone never sees (function declarations hoist above them).
        // Collect the signatures inside a seeded environment (imported bindings +
        // the module's inferred value symbols, mirroring the check phase's merged
        // environment) so the exported function types carry real parameter types.
        // The seed is environment-only: `local_symbols` keeps just what collection
        // itself declared (the file's functions and classes) — a leaked seed would
        // re-report the declaration as TS2451 when the check phase declares it
        // again. Declaration files skip the seeding: their exports resolve through
        // the declaration tables, and running initializer inference over large
        // dependency `.d.ts` files here would be pure cost.
        // The per-file scope fallback (`module_scope_by_file`) is live only for
        // SIGNATURE collection: a parameter typed through a local alias of an
        // imported qualified type (`type BtnProps = React.ComponentProps<…>`)
        // must not bake a degraded signature into the module's symbols and
        // export table (the alias's attached scope carries no import layers).
        // Value collection and export-table construction stay map-less: with the
        // fallback live they eagerly materialize every exported initializer of a
        // large cyclic program (zod: +11s/+380MB), and their degraded shapes are
        // re-resolved lazily by the check phase anyway.
        let saved_module_scope_by_file = std::mem::take(&mut ctx.module_scope_by_file);
        let mut signature_env = SymbolTable::new();
        let mut seeded_names: std::collections::HashSet<Arc<str>> = std::collections::HashSet::new();
        if !parsed_file.file_kind.is_declaration() {
            let mut import_seed = SymbolTable::new();
            if let Some(bindings) = preliminary_module_import_bindings[file_index].as_ref() {
                for (name, symbol) in bindings.symbols.iter_shared() {
                    let _ = import_seed.insert_shared(name.clone(), symbol.clone());
                }
            }
            let value_env = crate::modules::collect_exportable_value_symbols(
                &parsed_file.statements,
                local_type_declarations.as_ref(),
                &import_seed,
                ctx,
            );
            for (name, symbol) in value_env.iter_shared() {
                let _ = signature_env.insert_shared(name.clone(), symbol.clone());
                seeded_names.insert(name.clone());
            }
        }
        ctx.module_scope_by_file = saved_module_scope_by_file;
        let mut local_function_signatures = HashMap::new();
        let diagnostics_before_signatures = ctx.diagnostics().len();
        collect_function_signatures_from_statements(
            &parsed_file.statements,
            file_index,
            &mut signature_env,
            &mut local_function_signatures,
            ctx,
        );
        let saved_module_scope_by_file = std::mem::take(&mut ctx.module_scope_by_file);
        let mut local_symbols = SymbolTable::new();
        for (name, symbol) in signature_env.iter_shared() {
            if !seeded_names.contains(name) {
                let _ = local_symbols.insert_shared(name.clone(), symbol.clone());
            }
        }
        ctx.truncate_diagnostics(diagnostics_before_signatures);
        ctx.resolved_named_types = Arc::new(Mutex::new(HashMap::new()));

        // Lower this module's `declare global` augmentation values now that its
        // type environment (local declarations + import scope) is active. The
        // augmentation types were merged globally before binding, so a value such
        // as `var Buffer: BufferConstructor` sees the fully-merged interface while
        // `var x: ImportedType` still resolves through the module's imports.
        lower_global_augmentation_values_from_statements(&parsed_file.statements, ctx);

        let imported_symbols = preliminary_module_import_bindings[file_index]
            .as_ref()
            .map(|bindings| &bindings.symbols);
        let empty_imported_symbols = SymbolTable::new();
        let export_table = build_module_export_table(
            parsed_file,
            local_type_declarations.as_ref(),
            &local_symbols,
            imported_symbols.unwrap_or(&empty_imported_symbols),
            Some(full_type_declarations_scope),
            ctx,
        );
        ctx.module_scope_by_file = saved_module_scope_by_file;

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
                layers.extend(imported.scope_layers());
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
                return surge_ts_types::with_type_copy_reason(
                    surge_ts_types::TypeCopyReason::ModuleExport,
                    || fallback_bindings.clone(),
                );
            };

            let Some(fallback_bindings) = fallback_bindings else {
                return Some(
                    final_bindings.clone_with_reason(surge_ts_types::TypeCopyReason::ModuleExport),
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
            let mut merged_bindings =
                final_bindings.clone_with_reason(surge_ts_types::TypeCopyReason::ModuleExport);
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
