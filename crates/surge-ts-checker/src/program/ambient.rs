//! Ambient global and ambient-module (`declare module "..."`) collection passes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use surge_ts_syntax::ParsedStatement;

use super::*;

use crate::context::{CheckerContext, FileKind};
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
    // Phase 1: collect and merge every ambient global *type* declaration across
    // all declaration files before any value symbol is lowered. The default lib
    // graph splits a single global interface across files (e.g. `ArrayConstructor`
    // gains `isArray` in lib.es5 and `from`/`of` in lib.es2015.core), and a
    // `declare var Array: ArrayConstructor` would otherwise freeze the variable's
    // type against whatever members were merged when its own file was processed,
    // dropping members contributed by files processed later.
    for parsed_file in parsed_files {
        if !is_ambient_global_declaration_file(parsed_file, ctx) {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;

        let shared =
            TypeDeclarationTable::with_arena(ctx.ambient_global_type_declarations.arena_handle());
        let saved_type_declarations = std::mem::replace(&mut ctx.type_declarations, shared);
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
            timings.dependency_declaration_collection += collect_duration;
            timings.dependency_declaration_lower_time += collect_duration;
        });

        // Declaration merging across global declaration files: the same
        // interface (a default lib's `Window`, or a project's split global
        // `interface Env`) contributes members from every declaration rather
        // than being dropped first-wins.
        crate::symbols::merge_shared_arena_table_into(
            &mut ctx.ambient_global_type_declarations,
            &ambient_td,
        );

        ctx.type_declarations = saved_type_declarations;
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }

    // Phase 2: lower ambient value symbols (functions, `declare var`s, and
    // `declare class` constructors) against the now fully-merged type table, so
    // a variable typed by a split global interface sees every member.
    for parsed_file in parsed_files {
        if !is_ambient_global_declaration_file(parsed_file, ctx) {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;
        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());

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
                    surge_ts_syntax::ParsedExportDeclaration::Statement { declaration, .. },
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
                    .unwrap_or(surge_ts_types::Type::Unknown);
                if ctx.ambient_global_symbols.get(&var.name).is_none() {
                    ctx.ambient_global_symbols.insert(
                        var.name.clone(),
                        crate::symbols::SymbolInfo {
                            ty,
                            kind: if matches!(var.kind, surge_ts_syntax::ParsedVariableKind::Const)
                            {
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
                    surge_ts_syntax::ParsedExportDeclaration::Default {
                        declaration: surge_ts_syntax::ParsedDefaultExportDeclaration::Function(f),
                        ..
                    },
                ) => f.name.clone(),
                ParsedStatement::ExportDeclaration(
                    surge_ts_syntax::ParsedExportDeclaration::Statement { declaration, .. },
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
                        ty: surge_ts_types::Type::Function(fun_ty),
                        kind: crate::symbols::SymbolKind::Function,
                        function_signature: None,
                    },
                );
            }
        }

        // `declare class` contributes a global constructor/static value. The
        // instance interface is already in `ambient_global_type_declarations`
        // above, so the value's construct signature and member types resolve.
        for stmt in &parsed_file.statements {
            let class = match stmt {
                ParsedStatement::ClassDeclaration(class) => Some(class),
                ParsedStatement::ExportDeclaration(
                    surge_ts_syntax::ParsedExportDeclaration::Statement { declaration, .. },
                ) => {
                    if let ParsedStatement::ClassDeclaration(class) = declaration.as_ref() {
                        Some(class)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(class) = class {
                if ctx.ambient_global_symbols.get(&class.name).is_none() {
                    let symbol = super::build_class_value_symbol(class, ctx);
                    ctx.ambient_global_symbols
                        .insert(class.name.clone(), symbol);
                }
            }
        }

        ctx.type_declarations = saved_type_declarations;
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }
}

/// Whether a parsed file contributes to the ambient global scope. Declaration
/// files do, except dependency declarations that are not part of a configured
/// `@types/*` package (those reach the program only through module resolution).
fn is_ambient_global_declaration_file(
    parsed_file: &ParsedProgramFile,
    ctx: &CheckerContext,
) -> bool {
    if !parsed_file.file_kind.is_declaration() {
        return false;
    }

    if parsed_file.file_kind == FileKind::DependencyDeclaration
        && !is_configured_types_global_file(&parsed_file.file_name, &ctx.options.types)
    {
        return false;
    }

    true
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
                                .unwrap_or(surge_ts_types::Type::Unknown);
                            ctx.symbols.insert(
                                var.name.clone(),
                                crate::symbols::SymbolInfo {
                                    kind: if matches!(
                                        var.kind,
                                        surge_ts_syntax::ParsedVariableKind::Const
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
                        surge_ts_syntax::ParsedExportDeclaration::Statement { declaration, .. },
                    ) => {
                        if let ParsedStatement::VariableDeclaration(var) = declaration.as_ref() {
                            if ctx.symbols.get(&var.name).is_none() {
                                let ty = var
                                    .declared_type
                                    .as_ref()
                                    .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                                    .unwrap_or(surge_ts_types::Type::Unknown);
                                ctx.symbols.insert(
                                    var.name.clone(),
                                    crate::symbols::SymbolInfo {
                                        kind: if matches!(
                                            var.kind,
                                            surge_ts_syntax::ParsedVariableKind::Const
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
                &SymbolTable::new(),
                Some(current_type_declarations_scope),
                ctx,
            );
            let lowered_type_declarations = current_type_declarations.len() as u64;
            ctx.type_declarations = current_type_declarations;
            ctx.symbols = current_symbols;

            if parsed_file.is_module {
                // `declare module "x"` inside a module file augments an existing
                // module rather than declaring a new ambient one. It is merged
                // into the resolved target on import, never made resolvable here.
                match ctx.module_augmentations.get_mut(&module.module_specifier) {
                    Some(existing) => merge_module_export_tables(existing, &raw_export_table),
                    None => {
                        ctx.module_augmentations
                            .insert(module.module_specifier.clone(), raw_export_table);
                    }
                }
            } else if let Some(existing_index) = ambient_module_indexes
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

/// Merge a module augmentation into an already-resolved target export table.
///
/// Augmented interfaces merge their members into the target's existing exports
/// (declaration merging); new exported values and types are added. The target's
/// namespace export shape is preserved, since the augmentation only extends it.
pub(crate) fn apply_module_augmentation(
    base: &mut ModuleExportTable,
    augmentation: &ModuleExportTable,
) {
    for (name, declaration) in augmentation.type_declarations.iter() {
        crate::symbols::merge_type_declaration_into_table(
            &mut base.type_declarations,
            name.as_ref(),
            declaration,
        );
    }

    for (name, symbol) in augmentation.symbols.iter_shared() {
        if base.symbols.get(name).is_none() {
            let _ = base.symbols.insert_shared(name.clone(), symbol.clone());
        }
    }
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
        crate::symbols::merge_type_declaration_into_table(
            &mut target.type_declarations,
            name.as_ref(),
            declaration,
        );
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
