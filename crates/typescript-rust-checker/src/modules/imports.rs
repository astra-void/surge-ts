//! Resolving `import` declarations into local type and value bindings.

use super::*;

use std::sync::Arc;
use std::time::Instant;

use typescript_rust_syntax::{
    ParsedImportDeclaration, ParsedImportKind, ParsedStatement, ParsedType,
};
use typescript_rust_types::{Type, TypeCopyReason};

use crate::context::CheckerContext;
use crate::program::{ParsedProgramFile, record_program_timing};
use crate::symbols::{
    SymbolInfo, SymbolKind, SymbolTable, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationScope,
    TypeDeclarationTable,
};

pub(crate) fn resolve_module_imports(
    parsed_file: &ParsedProgramFile,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> ModuleImportBindings {
    let mut type_declarations = TypeDeclarationTable::new();
    let mut symbols = SymbolTable::new();

    for statement in &parsed_file.statements {
        let ParsedStatement::ImportDeclaration(import) = statement else {
            continue;
        };

        resolve_import_declaration(
            import,
            program_files,
            module_export_tables,
            module_resolution_scopes,
            local_symbols,
            &mut type_declarations,
            &mut symbols,
            ctx,
        );
    }

    ModuleImportBindings {
        type_declarations: Arc::new(type_declarations),
        symbols,
    }
}

pub(crate) fn try_resolve_module(
    module_specifier: &str,
    ctx: &CheckerContext,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
) -> Option<(
    ModuleExportTable,
    Option<Arc<TypeDeclarationScope>>,
    Option<usize>,
)> {
    let resolution_start = Instant::now();
    if let Some(resolved_file_name) = ctx.options.resolved_modules.get(module_specifier) {
        let resolved_file_name = canonical_file_identity(resolved_file_name);
        if let Some(resolved_index) = ctx
            .module_file_index_by_identity
            .get(resolved_file_name.as_str())
        {
            if let Some(Some(export_table)) = module_export_tables.get(*resolved_index) {
                let scope = module_resolution_scopes
                    .get(*resolved_index)
                    .and_then(|scope| scope.clone());
                record_program_timing(ctx.timings.as_ref(), |timings| {
                    timings.export_table_lookup += resolution_start.elapsed();
                    timings.package_export_lookup += resolution_start.elapsed();
                    timings.import_specifier_resolution += resolution_start.elapsed();
                });
                return Some((
                    export_table.clone_with_reason(TypeCopyReason::ModuleExport),
                    scope,
                    Some(*resolved_index),
                ));
            }
        }
    }

    if let Some(export_table) = ctx.ambient_modules.get(module_specifier) {
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.package_export_lookup += resolution_start.elapsed();
            timings.import_specifier_resolution += resolution_start.elapsed();
        });
        return Some((
            export_table.clone_with_reason(TypeCopyReason::ModuleExport),
            Some(Arc::new(TypeDeclarationScope::new(vec![Arc::new(
                ctx.type_declarations.clone(),
            )]))),
            None,
        ));
    }

    if let Some(resolved) = resolve_relative_module(
        &ctx.file_name,
        module_specifier,
        program_files,
        &ctx.module_file_index_by_identity,
    ) {
        if let Some(Some(export_table)) = module_export_tables.get(resolved.resolved_file_index) {
            let scope = module_resolution_scopes
                .get(resolved.resolved_file_index)
                .and_then(|scope| scope.clone());
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.export_table_lookup += resolution_start.elapsed();
                timings.import_specifier_resolution += resolution_start.elapsed();
            });
            return Some((
                export_table.clone_with_reason(TypeCopyReason::ModuleExport),
                scope,
                Some(resolved.resolved_file_index),
            ));
        }
    }

    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.import_specifier_resolution += resolution_start.elapsed()
    });
    None
}

pub(crate) fn resolve_import_declaration(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    match &import.kind {
        ParsedImportKind::Unsupported => {
            if !is_declaration_file_name(&ctx.file_name) {
                emit_unsupported_module_syntax_diagnostic(ctx, import);
            }
            return;
        }
        ParsedImportKind::DefaultAndNamed { .. } => resolve_default_and_named_import(
            import,
            program_files,
            module_export_tables,
            module_resolution_scopes,
            local_symbols,
            type_declarations,
            symbols,
            ctx,
        ),
        ParsedImportKind::Default { .. } => resolve_default_import(
            import,
            program_files,
            module_export_tables,
            module_resolution_scopes,
            local_symbols,
            symbols,
            ctx,
        ),
        ParsedImportKind::Namespace { .. } => resolve_namespace_import(
            import,
            program_files,
            module_export_tables,
            module_resolution_scopes,
            local_symbols,
            type_declarations,
            symbols,
            ctx,
        ),
        ParsedImportKind::SideEffect => {
            if ctx.ambient_modules.contains_key(&import.module_specifier) {
                return;
            }
            if ctx
                .options
                .resolved_modules
                .contains_key(&import.module_specifier)
            {
                return;
            }
            if resolve_relative_module(
                &ctx.file_name,
                &import.module_specifier,
                program_files,
                &ctx.module_file_index_by_identity,
            )
            .is_none()
            {
                if !(ctx.options.stub_external_modules
                    && is_external_specifier(&import.module_specifier))
                {
                    emit_unresolved_module_diagnostic(ctx, import);
                }
            }
            return;
        }
        ParsedImportKind::Named { .. } => resolve_named_import(
            import,
            program_files,
            module_export_tables,
            module_resolution_scopes,
            type_declarations,
            symbols,
            ctx,
        ),
    };
}

fn resolve_default_and_named_import(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    let ParsedImportKind::DefaultAndNamed {
        local_name,
        name_span,
        is_type_only,
        specifiers,
    } = &import.kind
    else {
        return;
    };
    let Some((export_table, _, resolved_index)) = try_resolve_module(
        &import.module_specifier,
        ctx,
        program_files,
        module_export_tables,
        module_resolution_scopes,
    ) else {
        if resolve_relative_module(
            &ctx.file_name,
            &import.module_specifier,
            program_files,
            &ctx.module_file_index_by_identity,
        )
        .is_none()
        {
            if !(ctx.options.stub_external_modules
                && is_external_specifier(&import.module_specifier))
            {
                emit_unresolved_module_diagnostic(ctx, import);
            }
        } else {
            emit_missing_export_diagnostic(ctx, &import.module_specifier, "default", *name_span);
        }

        if *is_type_only {
            let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                name: local_name.clone(),
                file_name: ctx.file_name.clone(),
                name_span: *name_span,
                type_parameters: vec![],
                ty: ParsedType::Unknown,
                resolution_scope: None,
            });
            if type_declarations.get(local_name).is_none() {
                let _ = type_declarations.insert(local_name.clone(), declaration);
            }
        } else {
            insert_unknown_value_import(local_name, symbols);
        }

        for specifier in specifiers {
            if *is_type_only {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                    name: specifier.local_name.to_string(),
                    file_name: ctx.file_name.clone(),
                    name_span: specifier.name_span,
                    type_parameters: vec![],
                    ty: ParsedType::Unknown,
                    resolution_scope: None,
                });
                if type_declarations.get(&specifier.local_name).is_none() {
                    let _ = type_declarations.insert(specifier.local_name.clone(), declaration);
                }
            } else {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                    name: specifier.local_name.to_string(),
                    file_name: ctx.file_name.clone(),
                    name_span: specifier.name_span,
                    type_parameters: vec![],
                    ty: ParsedType::Unknown,
                    resolution_scope: None,
                });
                if type_declarations.get(&specifier.local_name).is_none() {
                    let _ = type_declarations.insert(specifier.local_name.clone(), declaration);
                }
                insert_unknown_value_import(&specifier.local_name, symbols);
            }
        }
        return;
    };

    let Some(default_symbol) = export_table.get_shared_value("default") else {
        if should_bind_unknown_for_missing_export(&export_table, resolved_index, program_files) {
            if *is_type_only {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                    name: local_name.clone(),
                    file_name: ctx.file_name.clone(),
                    name_span: *name_span,
                    type_parameters: vec![],
                    ty: ParsedType::Unknown,
                    resolution_scope: None,
                });
                if type_declarations.get(local_name).is_none() {
                    let _ = type_declarations.insert(local_name.clone(), declaration);
                }
            } else {
                insert_unknown_value_import(local_name, symbols);
            }
            return;
        }

        emit_missing_export_diagnostic(ctx, &import.module_specifier, "default", *name_span);
        if *is_type_only {
            let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                name: local_name.clone(),
                file_name: ctx.file_name.clone(),
                name_span: *name_span,
                type_parameters: vec![],
                ty: ParsedType::Unknown,
                resolution_scope: None,
            });
            if type_declarations.get(local_name).is_none() {
                let _ = type_declarations.insert(local_name.clone(), declaration);
            }
        } else {
            insert_unknown_value_import(local_name, symbols);
        }
        return;
    };

    if *is_type_only {
        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: local_name.clone(),
            file_name: ctx.file_name.clone(),
            name_span: *name_span,
            type_parameters: vec![],
            ty: ParsedType::Unknown,
            resolution_scope: None,
        });
        if type_declarations.get(local_name).is_none() {
            let _ = type_declarations.insert(local_name.clone(), declaration);
        }
    } else if local_symbols.get(local_name).is_none() {
        symbols.insert_shared(local_name.clone(), default_symbol);
    }

    for specifier in specifiers {
        let type_export = lookup_type_export(&export_table, &specifier.local_name);
        let value_export = lookup_value_export(&export_table, &specifier.local_name);

        if *is_type_only {
            if let Some(type_export) = type_export {
                export_local_type_declaration(
                    type_export,
                    &specifier.local_name,
                    None,
                    type_declarations,
                );
                continue;
            }

            emit_missing_named_import_diagnostic(
                ctx,
                &import.module_specifier,
                &specifier.imported_name,
                specifier.name_span,
                module_has_explicit_default_export(
                    &import.module_specifier,
                    resolved_index,
                    program_files,
                    ctx,
                ),
            );
            insert_unknown_type_import(
                type_declarations,
                &specifier.local_name,
                ctx.file_name.clone(),
                specifier.name_span,
            );
            continue;
        }

        let mut found = false;

        if let Some(type_export) = type_export {
            export_local_type_declaration(
                type_export,
                &specifier.local_name,
                None,
                type_declarations,
            );
            found = true;
        }

        if let Some(value_export) = value_export {
            if symbols.get(&specifier.local_name).is_none() {
                symbols.insert_shared(specifier.local_name.clone(), value_export);
            }
            found = true;
        }

        if !found {
            if should_bind_unknown_for_missing_export(&export_table, resolved_index, program_files)
            {
                insert_unknown_type_import(
                    type_declarations,
                    &specifier.local_name,
                    ctx.file_name.clone(),
                    specifier.name_span,
                );
                insert_unknown_value_import(&specifier.local_name, symbols);
                continue;
            }

            emit_missing_named_import_diagnostic(
                ctx,
                &import.module_specifier,
                &specifier.imported_name,
                specifier.name_span,
                module_has_explicit_default_export(
                    &import.module_specifier,
                    resolved_index,
                    program_files,
                    ctx,
                ),
            );
            insert_unknown_type_import(
                type_declarations,
                &specifier.local_name,
                ctx.file_name.clone(),
                specifier.name_span,
            );
            insert_unknown_value_import(&specifier.local_name, symbols);
        }
    }
    return;
}

fn resolve_default_import(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    let ParsedImportKind::Default {
        local_name,
        name_span,
    } = &import.kind
    else {
        return;
    };
    let Some((export_table, _, resolved_index)) = try_resolve_module(
        &import.module_specifier,
        ctx,
        program_files,
        module_export_tables,
        module_resolution_scopes,
    ) else {
        if resolve_relative_module(
            &ctx.file_name,
            &import.module_specifier,
            program_files,
            &ctx.module_file_index_by_identity,
        )
        .is_none()
        {
            if !(ctx.options.stub_external_modules
                && is_external_specifier(&import.module_specifier))
            {
                emit_unresolved_module_diagnostic(ctx, import);
            }
        } else {
            emit_missing_export_diagnostic(ctx, &import.module_specifier, "default", *name_span);
        }
        insert_unknown_value_import(local_name, symbols);
        return;
    };

    let Some(default_symbol) = export_table.get_shared_value("default") else {
        if allows_synthetic_default_import(resolved_index, program_files) {
            if local_symbols.get(local_name).is_none() {
                symbols.insert(
                    local_name.clone(),
                    SymbolInfo {
                        ty: Type::Any,
                        kind: SymbolKind::Const,
                        function_signature: None,
                    },
                );
            }
            return;
        }

        if should_bind_unknown_for_missing_export(&export_table, resolved_index, program_files) {
            insert_unknown_value_import(local_name, symbols);
            return;
        }

        emit_missing_export_diagnostic(ctx, &import.module_specifier, "default", *name_span);
        insert_unknown_value_import(local_name, symbols);
        return;
    };

    if local_symbols.get(local_name).is_none() {
        symbols.insert_shared(local_name.clone(), default_symbol);
    }
    return;
}

fn resolve_namespace_import(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    let ParsedImportKind::Namespace {
        local_name,
        name_span: _,
        is_type_only,
    } = &import.kind
    else {
        return;
    };
    if *is_type_only {
        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: local_name.clone(),
            file_name: ctx.file_name.clone(),
            name_span: None,
            type_parameters: vec![],
            ty: ParsedType::Unknown,
            resolution_scope: None,
        });
        if type_declarations.get(local_name).is_none() {
            let _ = type_declarations.insert(local_name.clone(), declaration);
        }
        return;
    }

    let namespace_type = if let Some((export_table, _, _)) = try_resolve_module(
        &import.module_specifier,
        ctx,
        program_files,
        module_export_tables,
        module_resolution_scopes,
    ) {
        namespace_export_object_type(&export_table)
    } else {
        if resolve_relative_module(
            &ctx.file_name,
            &import.module_specifier,
            program_files,
            &ctx.module_file_index_by_identity,
        )
        .is_none()
        {
            if !(ctx.options.stub_external_modules
                && is_external_specifier(&import.module_specifier))
            {
                emit_unresolved_module_diagnostic(ctx, import);
            }
        }
        insert_unknown_value_import(local_name, symbols);
        return;
    };

    if local_symbols.get(local_name).is_none() {
        symbols.insert(
            local_name.clone(),
            SymbolInfo {
                ty: namespace_type,
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }
    return;
}

fn resolve_named_import(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    let ParsedImportKind::Named {
        is_type_only,
        specifiers,
    } = &import.kind
    else {
        return;
    };
    let Some((export_table, scope, resolved_index)) = try_resolve_module(
        &import.module_specifier,
        ctx,
        program_files,
        module_export_tables,
        module_resolution_scopes,
    ) else {
        let Some(resolved) = resolve_relative_module(
            &ctx.file_name,
            &import.module_specifier,
            program_files,
            &ctx.module_file_index_by_identity,
        ) else {
            if !(ctx.options.stub_external_modules
                && is_external_specifier(&import.module_specifier))
            {
                emit_unresolved_module_diagnostic(ctx, import);
            }
            for specifier in specifiers {
                if *is_type_only {
                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    continue;
                }

                insert_unknown_type_import(
                    type_declarations,
                    &specifier.local_name,
                    ctx.file_name.clone(),
                    specifier.name_span,
                );
                insert_unknown_value_import(&specifier.local_name, symbols);
            }
            return;
        };

        let local_scope = module_resolution_scopes
            .get(resolved.resolved_file_index)
            .and_then(|scope| scope.clone());

        for specifier in specifiers {
            if let Some(local_scope) = &local_scope {
                if let Some(local_declaration) = local_scope.get(&specifier.imported_name).cloned()
                {
                    if *is_type_only {
                        insert_type_export(
                            type_declarations,
                            &specifier.local_name,
                            Some(&local_scope),
                            local_declaration,
                        );
                        continue;
                    }

                    insert_type_export(
                        type_declarations,
                        &specifier.local_name,
                        Some(&local_scope),
                        local_declaration,
                    );
                    continue;
                }
            }

            emit_missing_named_import_diagnostic(
                ctx,
                &import.module_specifier,
                &specifier.imported_name,
                specifier.name_span,
                module_has_explicit_default_export(
                    &import.module_specifier,
                    None,
                    program_files,
                    ctx,
                ),
            );

            insert_unknown_type_import(
                type_declarations,
                &specifier.local_name,
                ctx.file_name.clone(),
                specifier.name_span,
            );
            insert_unknown_value_import(&specifier.local_name, symbols);
        }
        return;
    };

    let Some(scope) = scope else {
        let local_scope = module_resolution_scopes
            .get(resolved_index.unwrap_or(usize::MAX))
            .and_then(|scope| scope.clone());

        for specifier in specifiers {
            if let Some(local_scope) = &local_scope {
                if let Some(local_declaration) = local_scope.get(&specifier.imported_name).cloned()
                {
                    if *is_type_only {
                        insert_type_export(
                            type_declarations,
                            &specifier.local_name,
                            Some(&local_scope),
                            local_declaration,
                        );
                        continue;
                    }

                    insert_type_export(
                        type_declarations,
                        &specifier.local_name,
                        Some(&local_scope),
                        local_declaration,
                    );
                    continue;
                }
            }

            emit_missing_export_diagnostic(
                ctx,
                &import.module_specifier,
                &specifier.imported_name,
                specifier.name_span,
            );

            if *is_type_only {
                insert_unknown_type_import(
                    type_declarations,
                    &specifier.local_name,
                    ctx.file_name.clone(),
                    specifier.name_span,
                );
                continue;
            }

            insert_unknown_type_import(
                type_declarations,
                &specifier.local_name,
                ctx.file_name.clone(),
                specifier.name_span,
            );
            insert_unknown_value_import(&specifier.local_name, symbols);
        }
        return;
    };

    let has_unresolved_star_export = export_table.has_unresolved_star_export
        || resolved_index
            .map(|i| {
                module_has_unresolved_star_export(
                    i,
                    program_files,
                    &ctx.module_file_index_by_identity,
                )
            })
            .unwrap_or(false);

    for specifier in specifiers {
        let type_export = lookup_type_export(&export_table, &specifier.imported_name);
        let value_export = lookup_value_export(&export_table, &specifier.imported_name);
        if *is_type_only {
            if let Some(type_export) = type_export {
                insert_type_export(
                    type_declarations,
                    &specifier.local_name,
                    Some(&scope),
                    type_export.clone(),
                );
                continue;
            }

            if let Some(local_declaration) = scope.get(&specifier.imported_name).cloned() {
                insert_type_export(
                    type_declarations,
                    &specifier.local_name,
                    Some(&scope),
                    local_declaration,
                );
                continue;
            }

            emit_missing_export_diagnostic(
                ctx,
                &import.module_specifier,
                &specifier.imported_name,
                specifier.name_span,
            );
            insert_unknown_type_import(
                type_declarations,
                &specifier.local_name,
                ctx.file_name.clone(),
                specifier.name_span,
            );
            continue;
        }

        let mut found = false;

        if let Some(type_export) = type_export {
            insert_type_export(
                type_declarations,
                &specifier.local_name,
                Some(&scope),
                type_export.clone(),
            );
            found = true;
        } else if let Some(local_declaration) = scope.get(&specifier.imported_name).cloned() {
            insert_type_export(
                type_declarations,
                &specifier.local_name,
                Some(&scope),
                local_declaration,
            );
            found = true;
        }

        if let Some(value_export) = value_export {
            symbols.insert_shared(specifier.local_name.clone(), value_export);
            found = true;
        }

        if !found {
            if has_unresolved_star_export {
                if *is_type_only {
                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    continue;
                }

                insert_unknown_type_import(
                    type_declarations,
                    &specifier.local_name,
                    ctx.file_name.clone(),
                    specifier.name_span,
                );
                insert_unknown_value_import(&specifier.local_name, symbols);
                continue;
            }

            if should_bind_unknown_for_missing_export(&export_table, resolved_index, program_files)
            {
                insert_unknown_type_import(
                    type_declarations,
                    &specifier.local_name,
                    ctx.file_name.clone(),
                    specifier.name_span,
                );
                insert_unknown_value_import(&specifier.local_name, symbols);
                continue;
            }

            emit_missing_export_diagnostic(
                ctx,
                &import.module_specifier,
                &specifier.imported_name,
                specifier.name_span,
            );
            insert_unknown_type_import(
                type_declarations,
                &specifier.local_name,
                ctx.file_name.clone(),
                specifier.name_span,
            );
            insert_unknown_value_import(&specifier.local_name, symbols);
        }
    }
    return;
}
