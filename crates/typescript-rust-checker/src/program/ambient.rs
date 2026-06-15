//! Ambient global and ambient-module (`declare module "..."`) collection passes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use typescript_rust_syntax::ParsedStatement;

use super::*;

use crate::context::{CheckerContext, FileKind};
use crate::default_lib::inject_generated_default_lib_snapshot_for_file_name;
use crate::driver::collect_type_declarations;
use crate::modules::{ModuleExportTable, build_module_export_table};
use crate::symbols::{SymbolTable, TypeDeclarationScope, TypeDeclarationTable};

#[derive(Debug, Clone)]
pub(crate) struct AmbientModuleEntry {
    module_specifier: String,
    file: ParsedProgramFile,
    raw_export_table: ModuleExportTable,
}

pub(crate) fn collect_ambient_globals(
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

        // Dependency declaration files normally contribute symbols only through
        // module resolution, not the global scope. Configured `@types/*`
        // packages (`compilerOptions.types`) are the exception: like TypeScript,
        // their non-module declarations populate the ambient global scope.
        if parsed_file.file_kind == FileKind::DependencyDeclaration
            && !is_configured_types_global_file(&parsed_file.file_name, &ctx.options.types)
        {
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

/// Whether `file_name` belongs to one of the configured `compilerOptions.types`
/// packages under `node_modules/@types/<mangled>`. Scoped names map like
/// TypeScript: `@scope/pkg` -> `scope__pkg`.
fn is_configured_types_global_file(file_name: &str, types: &[String]) -> bool {
    types.iter().any(|type_name| {
        let mangled = mangle_types_package_name(type_name);
        let needle = format!("/@types/{mangled}/");
        file_name.contains(&needle)
    })
}

fn mangle_types_package_name(type_name: &str) -> String {
    type_name
        .strip_prefix('@')
        .map(|name| name.replace('/', "__"))
        .unwrap_or_else(|| type_name.to_string())
}

pub(crate) fn collect_ambient_modules(
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

pub(crate) fn merge_module_export_tables(
    target: &mut ModuleExportTable,
    source: &ModuleExportTable,
) {
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
