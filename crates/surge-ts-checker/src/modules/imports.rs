//! Resolving `import` declarations into local type and value bindings.

use super::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use surge_ts_syntax::{ParsedImportDeclaration, ParsedImportKind, ParsedStatement, ParsedType};
use surge_ts_types::{Type, TypeCopyReason};

use crate::context::CheckerContext;
use crate::program::{ParsedProgramFile, record_program_timing};
use crate::symbols::{
    SymbolInfo, SymbolKind, SymbolTable, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationScope,
    TypeDeclarationTable,
};

/// Records that an external (non-relative) module specifier failed every
/// resolution path, for the `externalModuleStubs.unresolved` compatibility-report
/// figure. Relative specifiers are not external references and are not counted.
pub(crate) fn record_unresolved_external_module(ctx: &mut CheckerContext, specifier: &str) {
    if is_external_specifier(specifier) {
        ctx.stats.external_modules_unresolved_total += 1;
    }
}

/// Handles an import whose module specifier resolved to nothing: records it
/// against the unresolved-external figure, then emits the unresolved-module
/// diagnostic unless an external specifier is being intentionally stubbed
/// (`stub_external_modules`).
pub(crate) fn report_unresolved_module(ctx: &mut CheckerContext, import: &ParsedImportDeclaration) {
    record_unresolved_external_module(ctx, &import.module_specifier);
    if !(ctx.options.stub_external_modules && is_external_specifier(&import.module_specifier)) {
        emit_unresolved_module_diagnostic(ctx, import);
    }
}

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
    let mut namespace_alias_layers = Vec::new();

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
            &mut namespace_alias_layers,
            ctx,
        );
    }

    ModuleImportBindings {
        type_declarations: Arc::new(type_declarations),
        symbols,
        namespace_alias_layers,
    }
}

/// Looks up an ambient `declare module "…"` export table, honoring wildcard
/// patterns (`declare module "*.css"`, which Next's `next-env.d.ts` relies on for
/// `import "./globals.css"`). tsc matches a single `*` against any substring and
/// prefers the pattern with the longest matching prefix; an exact declaration
/// always wins.
pub(crate) fn ambient_module_export_table<'a>(
    ctx: &'a CheckerContext,
    module_specifier: &str,
) -> Option<&'a ModuleExportTable> {
    if let Some(export_table) = ctx.ambient_modules.get(module_specifier) {
        return Some(export_table);
    }

    let mut best: Option<(usize, &ModuleExportTable)> = None;
    for (pattern, export_table) in ctx.ambient_modules.iter() {
        let Some((prefix, suffix)) = pattern.split_once('*') else {
            continue;
        };
        if suffix.contains('*')
            || module_specifier.len() < prefix.len() + suffix.len()
            || !module_specifier.starts_with(prefix)
            || !module_specifier.ends_with(suffix)
        {
            continue;
        }
        if best.is_none_or(|(best_prefix, _)| prefix.len() > best_prefix) {
            best = Some((prefix.len(), export_table));
        }
    }

    best.map(|(_, export_table)| export_table)
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
    if let Some(resolved_file_name) = ctx
        .options
        .resolved_module_for(&ctx.file_name, module_specifier)
    {
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
                let mut export_table = export_table.clone_with_reason(TypeCopyReason::ModuleExport);
                if let Some(augmentation) = ctx.module_augmentations.get(module_specifier) {
                    crate::program::apply_module_augmentation(&mut export_table, augmentation);
                }
                return Some((export_table, scope, Some(*resolved_index)));
            }
        }
    }

    if let Some(export_table) = ambient_module_export_table(ctx, module_specifier) {
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.package_export_lookup += resolution_start.elapsed();
            timings.import_specifier_resolution += resolution_start.elapsed();
        });
        let mut export_table = export_table.clone_with_reason(TypeCopyReason::ModuleExport);
        if let Some(augmentation) = ctx.module_augmentations.get(module_specifier) {
            crate::program::apply_module_augmentation(&mut export_table, augmentation);
        }
        return Some((
            export_table,
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
    namespace_alias_layers: &mut Vec<Arc<TypeDeclarationTable>>,
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
            namespace_alias_layers,
            ctx,
        ),
        ParsedImportKind::Equals { .. } => resolve_import_equals(
            import,
            program_files,
            module_export_tables,
            module_resolution_scopes,
            local_symbols,
            symbols,
            ctx,
        ),
        ParsedImportKind::SideEffect => {
            if ambient_module_export_table(ctx, &import.module_specifier).is_some() {
                return;
            }
            if ctx
                .options
                .resolved_module_for(&ctx.file_name, &import.module_specifier)
                .is_some()
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
                report_unresolved_module(ctx, import);
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
            report_unresolved_module(ctx, import);
        } else {
            emit_missing_export_diagnostic(ctx, &import.module_specifier, "default", *name_span);
        }

        if *is_type_only {
            let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
                local_name.clone(),
                ctx.file_name_arc(),
                *name_span,
                vec![],
                ParsedType::Unknown,
                None,
            ));
            if type_declarations.get(local_name).is_none() {
                let _ = type_declarations.insert(local_name.clone(), declaration);
            }
        } else {
            insert_unknown_value_import(local_name, symbols);
        }

        for specifier in specifiers {
            if *is_type_only {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
                    specifier.local_name.to_string(),
                    ctx.file_name_arc(),
                    specifier.name_span,
                    vec![],
                    ParsedType::Unknown,
                    None,
                ));
                if type_declarations.get(&specifier.local_name).is_none() {
                    let _ = type_declarations.insert(specifier.local_name.clone(), declaration);
                }
            } else {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
                    specifier.local_name.to_string(),
                    ctx.file_name_arc(),
                    specifier.name_span,
                    vec![],
                    ParsedType::Unknown,
                    None,
                ));
                if type_declarations.get(&specifier.local_name).is_none() {
                    let _ = type_declarations.insert(specifier.local_name.clone(), declaration);
                }
                insert_unknown_value_import(&specifier.local_name, symbols);
            }
        }
        return;
    };

    // Bind the default specifier through the same default-import resolution it
    // would take on its own. A missing default emits TS2305 (unless the module
    // is an incomplete declaration surface) and binds an unknown placeholder,
    // but never returns early: the named specifiers below must still bind so a
    // missing default does not cascade into TS2304 on their usages.
    match export_table.get_shared_value("default") {
        Some(default_symbol) => {
            if *is_type_only {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
                    local_name.clone(),
                    ctx.file_name_arc(),
                    *name_span,
                    vec![],
                    ParsedType::Unknown,
                    None,
                ));
                if type_declarations.get(local_name).is_none() {
                    let _ = type_declarations.insert(local_name.clone(), declaration);
                }
            } else if local_symbols.get(local_name).is_none() {
                symbols.insert_shared(local_name.clone(), default_symbol);
            }
        }
        None => {
            if allows_synthetic_default_import(ctx, resolved_index, program_files) && !*is_type_only
            {
                bind_synthetic_default_import(local_name, local_symbols, symbols);
            } else if !should_bind_unknown_for_missing_export(
                &export_table,
                resolved_index,
                program_files,
            ) {
                emit_missing_export_diagnostic(
                    ctx,
                    &import.module_specifier,
                    "default",
                    *name_span,
                );
                if *is_type_only {
                    let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
                        local_name.clone(),
                        ctx.file_name_arc(),
                        *name_span,
                        vec![],
                        ParsedType::Unknown,
                        None,
                    ));
                    if type_declarations.get(local_name).is_none() {
                        let _ = type_declarations.insert(local_name.clone(), declaration);
                    }
                } else {
                    insert_unknown_value_import(local_name, symbols);
                }
            } else if *is_type_only {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
                    local_name.clone(),
                    ctx.file_name_arc(),
                    *name_span,
                    vec![],
                    ParsedType::Unknown,
                    None,
                ));
                if type_declarations.get(local_name).is_none() {
                    let _ = type_declarations.insert(local_name.clone(), declaration);
                }
            } else {
                insert_unknown_value_import(local_name, symbols);
            }
        }
    }

    for specifier in specifiers {
        // Named specifiers resolve against the module export by their imported
        // name; the local name is only the binding target (e.g. `helper as h`).
        let type_export = lookup_type_export(&export_table, &specifier.imported_name);
        let value_export = lookup_value_export(&export_table, &specifier.imported_name);

        let has_qualified_type_exports = copy_qualified_type_exports(
            &export_table,
            &specifier.imported_name,
            &specifier.local_name,
            type_declarations,
        );

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

            // A type-only namespace (`export namespace enumUtil { export type … }`)
            // has no direct export entry, only qualified `ns.Member` ones; the
            // import is still valid.
            if has_qualified_type_exports {
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
                ctx.file_name_arc(),
                specifier.name_span,
            );
            continue;
        }

        let mut found = has_qualified_type_exports;

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
                    ctx.file_name_arc(),
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
                ctx.file_name_arc(),
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
            report_unresolved_module(ctx, import);
        } else {
            emit_missing_export_diagnostic(ctx, &import.module_specifier, "default", *name_span);
        }
        insert_unknown_value_import(local_name, symbols);
        return;
    };

    let Some(default_symbol) = export_table.get_shared_value("default") else {
        if allows_synthetic_default_import(ctx, resolved_index, program_files) {
            bind_synthetic_default_import(local_name, local_symbols, symbols);
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

fn resolve_import_equals(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    let ParsedImportKind::Equals { local_name, .. } = &import.kind else {
        return;
    };

    let Some((export_table, _, _resolved_index)) = try_resolve_module(
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
            report_unresolved_module(ctx, import);
        }
        insert_unknown_value_import(local_name, symbols);
        return;
    };

    // A resolved module that exposes a supported `export = identifier` binds the
    // local name to that value. Otherwise bind an unknown placeholder without a
    // diagnostic so an unsupported/unresolved export target does not cascade.
    match export_table.export_assignment_symbol.clone() {
        Some(symbol) => {
            if local_symbols.get(local_name).is_none() {
                symbols.insert_shared(local_name.clone(), symbol);
            }
        }
        None => insert_unknown_value_import(local_name, symbols),
    }
}

fn bind_synthetic_default_import(
    local_name: &str,
    local_symbols: &SymbolTable,
    symbols: &mut SymbolTable,
) {
    if local_symbols.get(local_name).is_some() {
        return;
    }

    symbols.insert(
        local_name.to_string(),
        SymbolInfo {
            ty: Type::Any,
            kind: SymbolKind::Const,
            function_signature: None,
        },
    );
}

thread_local! {
    // `import * as ns from "m"` re-keys every type `m` exports under `ns.<member>`.
    // The result depends only on the resolved module and the alias, so a barrel
    // namespace-imported by many files (or the same module imported repeatedly)
    // would otherwise rebuild an O(exports) table per importer. Cached by
    // (resolved module index, alias) and cleared per run.
    static NAMESPACE_ALIAS_TABLE_CACHE: RefCell<HashMap<(usize, String), Arc<TypeDeclarationTable>>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn clear_namespace_alias_table_cache() {
    NAMESPACE_ALIAS_TABLE_CACHE.with(|cache| cache.borrow_mut().clear());
}

/// Build the `ns.<member>` type-declaration table for a namespace import. Mirrors
/// the per-member `insert_type_export` the eager path used, but produces a
/// standalone table that is appended to the resolution scope as a shared layer.
fn build_namespace_alias_table(
    export_table: &ModuleExportTable,
    local_name: &str,
    namespace_scope: Option<&Arc<TypeDeclarationScope>>,
) -> Arc<TypeDeclarationTable> {
    let mut table = TypeDeclarationTable::new();
    for (key, declaration) in export_table.type_declarations.iter() {
        // A member of an exported namespace is keyed `ns.Member`, and under a
        // namespace import its tsc-visible name keeps that qualifier
        // (`local.ns.Member`). Registering only the last segment leaves the real
        // name unresolvable, so the reference silently degrades to an open type.
        let local_key = format!("{local_name}.{key}");
        if table.get(&local_key).is_none() {
            crate::modules::exports::insert_type_export(
                &mut table,
                &local_key,
                namespace_scope,
                declaration.clone(),
            );
        }
    }
    Arc::new(table)
}

fn namespace_alias_table(
    export_table: &ModuleExportTable,
    local_name: &str,
    namespace_scope: Option<&Arc<TypeDeclarationScope>>,
    resolved_index: Option<usize>,
) -> Arc<TypeDeclarationTable> {
    let Some(index) = resolved_index else {
        return build_namespace_alias_table(export_table, local_name, namespace_scope);
    };

    let cache_key = (index, local_name.to_string());
    if let Some(cached) =
        NAMESPACE_ALIAS_TABLE_CACHE.with(|cache| cache.borrow().get(&cache_key).cloned())
    {
        return cached;
    }
    let table = build_namespace_alias_table(export_table, local_name, namespace_scope);
    NAMESPACE_ALIAS_TABLE_CACHE.with(|cache| {
        cache.borrow_mut().insert(cache_key, table.clone());
    });
    table
}

fn resolve_namespace_import(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    namespace_alias_layers: &mut Vec<Arc<TypeDeclarationTable>>,
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
        // `import type * as ns` still exposes the module's exported types under
        // the qualified alias (`ns.Member`), and `typeof ns.Member` stays legal
        // in type positions — only emitting the binding at runtime is elided. So
        // the namespace value shape is registered too, otherwise
        // `ComponentProps<typeof LabelPrimitive.Root>` reports a false TS2304.
        if let Some((export_table, scope, resolved_index)) = try_resolve_module(
            &import.module_specifier,
            ctx,
            program_files,
            module_export_tables,
            module_resolution_scopes,
        ) {
            namespace_alias_layers.push(namespace_alias_table(
                &export_table,
                local_name,
                scope.as_ref(),
                resolved_index,
            ));

            if local_symbols.get(local_name).is_none() {
                let namespace_type = namespace_export_object_type(&export_table);
                let namespace_type = match resolved_index.and_then(|index| program_files.get(index))
                {
                    Some(resolved_file) => {
                        tag_namespace_type_with_module_path(namespace_type, &resolved_file.file_name)
                    }
                    None => namespace_type,
                };
                symbols.insert(
                    local_name.clone(),
                    SymbolInfo {
                        ty: namespace_type,
                        kind: SymbolKind::Const,
                        function_signature: None,
                    },
                );
            }
        }

        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
            local_name.clone(),
            ctx.file_name_arc(),
            None,
            vec![],
            ParsedType::Unknown,
            None,
        ));
        if type_declarations.get(local_name).is_none() {
            let _ = type_declarations.insert(local_name.clone(), declaration);
        }
        return;
    }

    let (namespace_type, namespace_export_table, namespace_scope, namespace_resolved_index) =
        if let Some((export_table, scope, resolved_index)) = try_resolve_module(
            &import.module_specifier,
            ctx,
            program_files,
            module_export_tables,
            module_resolution_scopes,
        ) {
            let namespace_type = namespace_export_object_type(&export_table);
            // tsc displays a namespace import object as `typeof import("<path>")`
            // (absolute, without the source extension) rather than the structural
            // shape. Tag the object with that display form when we know the file.
            let namespace_type = match resolved_index.and_then(|index| program_files.get(index)) {
                Some(resolved_file) => {
                    tag_namespace_type_with_module_path(namespace_type, &resolved_file.file_name)
                }
                None => namespace_type,
            };
            (namespace_type, Some(export_table), scope, resolved_index)
        } else {
            if resolve_relative_module(
                &ctx.file_name,
                &import.module_specifier,
                program_files,
                &ctx.module_file_index_by_identity,
            )
            .is_none()
            {
                report_unresolved_module(ctx, import);
            }
            insert_unknown_value_import(local_name, symbols);
            return;
        };

    // Re-expose the module's exported types under the namespace alias so qualified
    // type references resolve (`React.ComponentProps<...>`, `M.SomeType`). Members
    // of an `export = <namespace>` keep their `<ns>.<member>` keys here; the first
    // segment is replaced with the local alias. Built once per (module, alias) and
    // appended as a shared scope layer rather than copied into every importer's
    // table, so `import * as` of a large barrel stays O(1) per importer.
    if let Some(export_table) = &namespace_export_table {
        namespace_alias_layers.push(namespace_alias_table(
            export_table,
            local_name,
            namespace_scope.as_ref(),
            namespace_resolved_index,
        ));
    }

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
            report_unresolved_module(ctx, import);
            for specifier in specifiers {
                if *is_type_only {
                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name_arc(),
                        specifier.name_span,
                    );
                    continue;
                }

                insert_unknown_type_import(
                    type_declarations,
                    &specifier.local_name,
                    ctx.file_name_arc(),
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
                ctx.file_name_arc(),
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
                    ctx.file_name_arc(),
                    specifier.name_span,
                );
                continue;
            }

            insert_unknown_type_import(
                type_declarations,
                &specifier.local_name,
                ctx.file_name_arc(),
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
        let has_qualified_type_exports = copy_qualified_type_exports(
            &export_table,
            &specifier.imported_name,
            &specifier.local_name,
            type_declarations,
        );
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

            // A type-only namespace exports only qualified `ns.Member` entries.
            if has_qualified_type_exports {
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
                ctx.file_name_arc(),
                specifier.name_span,
            );
            continue;
        }

        let mut found = has_qualified_type_exports;

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
                        ctx.file_name_arc(),
                        specifier.name_span,
                    );
                    continue;
                }

                insert_unknown_type_import(
                    type_declarations,
                    &specifier.local_name,
                    ctx.file_name_arc(),
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
                    ctx.file_name_arc(),
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
                ctx.file_name_arc(),
                specifier.name_span,
            );
            insert_unknown_value_import(&specifier.local_name, symbols);
        }
    }
    return;
}

/// Tags a namespace import object with tsc's `typeof import("<path>")` display
/// form. The path is the resolved module file made absolute and stripped of its
/// TypeScript extension (e.g. `…/pkg/index.d.ts` -> `…/pkg/index`).
fn tag_namespace_type_with_module_path(namespace_type: Type, resolved_file_name: &str) -> Type {
    match namespace_type {
        Type::Object(object) => {
            let path = strip_typescript_extension(resolved_file_name);
            Type::Object(object.with_alias_name(format!("typeof import(\"{path}\")")))
        }
        other => other,
    }
}

/// Strips a TypeScript source/declaration extension, matching the module name
/// tsc prints inside `typeof import(...)`. Declaration extensions are checked
/// first so `index.d.ts` becomes `index`, not `index.d`.
fn strip_typescript_extension(file_name: &str) -> &str {
    for extension in [".d.ts", ".d.mts", ".d.cts", ".ts", ".tsx", ".mts", ".cts"] {
        if let Some(stripped) = file_name.strip_suffix(extension) {
            return stripped;
        }
    }

    file_name
}
