//! Building and resolving per-module export tables (named, default, star, namespace).

use super::*;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use surge_ts_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedImportKind,
    ParsedNamespaceDeclaration, ParsedStatement, ParsedType, TextSpan,
};
use surge_ts_types::{FunctionType, ObjectProperty, PropertyMap, Type, TypeCopyReason};

use crate::checks::function as check_function;
use crate::checks::var::{VariableCheckOptions, check_variable_declaration_with_symbols};
use crate::context::{CheckerContext, FileKind};
use crate::program::{ParsedProgramFile, record_program_timing};
use crate::symbols::{
    SymbolInfo, SymbolKind, SymbolTable, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationScope,
    TypeDeclarationTable,
};

pub(crate) fn build_module_export_table(
    parsed_file: &ParsedProgramFile,
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    imported_symbols: &SymbolTable,
    resolution_scope: Option<Arc<TypeDeclarationScope>>,
    ctx: &mut CheckerContext,
) -> ModuleExportTable {
    let exportable_values = collect_exportable_value_symbols(
        &parsed_file.statements,
        local_type_declarations,
        local_symbols,
        ctx,
    );

    let mut type_declarations = TypeDeclarationTable::new();
    let mut symbols = SymbolTable::new();
    let mut default_symbol = None;
    let mut export_assignment_symbol = None;

    for statement in &parsed_file.statements {
        collect_exports_from_statement(
            statement,
            &exportable_values,
            imported_symbols,
            local_type_declarations,
            local_symbols,
            resolution_scope.as_ref(),
            &mut type_declarations,
            &mut symbols,
            &mut default_symbol,
            &mut export_assignment_symbol,
            ctx,
        );
    }

    ModuleExportTable {
        type_declarations: Arc::new(type_declarations),
        symbols,
        default_symbol,
        export_assignment_symbol,
        namespace_export_object_type: None,
        has_unresolved_star_export: false,
        has_incomplete_declaration_surface: module_has_incomplete_declaration_surface(parsed_file),
    }
}

pub(crate) fn resolve_module_export_tables(
    parsed_files: &[ParsedProgramFile],
    local_module_export_tables: &[Option<ModuleExportTable>],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleExportTable>> {
    let mut resolved_module_export_tables = vec![None; parsed_files.len()];
    let mut resolving = vec![false; parsed_files.len()];

    for file_index in 0..parsed_files.len() {
        let _ = resolve_module_export_table(
            file_index,
            parsed_files,
            local_module_export_tables,
            &mut resolved_module_export_tables,
            &mut resolving,
            ctx,
        );
    }

    resolved_module_export_tables
}

pub(crate) fn try_resolve_module_export_table(
    module_specifier: &str,
    ctx: &mut CheckerContext,
    parsed_files: &[ParsedProgramFile],
    local_module_export_tables: &[Option<ModuleExportTable>],
    resolved_module_export_tables: &mut [Option<ModuleExportTable>],
    resolving: &mut [bool],
    file_name: &str,
) -> Option<(ModuleExportTable, Option<usize>)> {
    let resolution_start = Instant::now();
    if let Some(resolved_file_name) = ctx.options.resolved_modules.get(module_specifier) {
        let resolved_file_name = canonical_file_identity(resolved_file_name);
        if let Some(resolved_index) = ctx
            .module_file_index_by_identity
            .get(resolved_file_name.as_str())
            .copied()
        {
            if let Some(export_table) = resolve_module_export_table(
                resolved_index,
                parsed_files,
                local_module_export_tables,
                resolved_module_export_tables,
                resolving,
                ctx,
            ) {
                record_program_timing(ctx.timings.as_ref(), |timings| {
                    timings.export_table_lookup += resolution_start.elapsed();
                    timings.package_export_lookup += resolution_start.elapsed();
                });
                return Some((export_table, Some(resolved_index)));
            }
        }
    }

    if let Some(export_table) = ctx.ambient_modules.get(module_specifier) {
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.package_export_lookup += resolution_start.elapsed();
        });
        return Some((export_table.clone(), None));
    }

    if let Some(resolved) = resolve_relative_module(
        file_name,
        module_specifier,
        parsed_files,
        &ctx.module_file_index_by_identity,
    ) {
        if let Some(export_table) = resolve_module_export_table(
            resolved.resolved_file_index,
            parsed_files,
            local_module_export_tables,
            resolved_module_export_tables,
            resolving,
            ctx,
        ) {
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.export_table_lookup += resolution_start.elapsed();
            });
            return Some((export_table, Some(resolved.resolved_file_index)));
        }
    }

    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.import_specifier_resolution += resolution_start.elapsed()
    });
    None
}

pub(crate) fn resolve_module_export_table(
    file_index: usize,
    parsed_files: &[ParsedProgramFile],
    local_module_export_tables: &[Option<ModuleExportTable>],
    resolved_module_export_tables: &mut [Option<ModuleExportTable>],
    resolving: &mut [bool],
    ctx: &mut CheckerContext,
) -> Option<ModuleExportTable> {
    // `file_index` may originate from a different vector domain than the slices
    // passed here. The relative/package resolvers and `module_file_index_by_identity`
    // index the full project file list, but some callers (ambient module binding)
    // pass a narrow local vector. A resolved index from the global domain can then
    // exceed these slices, so every access is bounds-checked and an out-of-domain
    // index yields a conservative unresolved result instead of panicking.
    if let Some(Some(resolved)) = resolved_module_export_tables.get(file_index) {
        return Some(resolved.clone());
    }

    let Some(parsed_file) = parsed_files.get(file_index) else {
        return None;
    };

    let Some(local_export_table) = local_module_export_tables
        .get(file_index)
        .and_then(|slot| slot.clone())
    else {
        return None;
    };

    match resolving.get(file_index) {
        Some(true) => return Some(local_export_table),
        Some(false) => {}
        None => return None,
    }

    if let Some(slot) = resolving.get_mut(file_index) {
        *slot = true;
    }
    ctx.set_file_name(parsed_file.file_name.clone());

    let mut resolved_export_table = local_export_table;

    for statement in &parsed_file.statements {
        let ParsedStatement::ExportDeclaration(export) = statement else {
            continue;
        };
        match export.as_ref() {
            ParsedExportDeclaration::Named {
                is_type_only,
                specifiers,
                module_specifier: Some(module_specifier),
                module_specifier_span,
                ..
            } => {
                let Some((target_export_table, resolved_index)) = try_resolve_module_export_table(
                    module_specifier,
                    ctx,
                    parsed_files,
                    local_module_export_tables,
                    resolved_module_export_tables,
                    resolving,
                    &parsed_file.file_name,
                ) else {
                    if resolve_relative_module(
                        &parsed_file.file_name,
                        module_specifier,
                        parsed_files,
                        &ctx.module_file_index_by_identity,
                    )
                    .is_none()
                    {
                        record_unresolved_external_module(ctx, module_specifier);
                        if !(ctx.options.stub_external_modules
                            && is_external_specifier(module_specifier))
                        {
                            emit_unresolved_export_module_diagnostic(
                                ctx,
                                module_specifier,
                                *module_specifier_span,
                            );
                        }
                    }

                    for specifier in specifiers {
                        let specifier_is_type_only = *is_type_only || specifier.is_type_only;
                        insert_unknown_type_import(
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );

                        if !specifier_is_type_only {
                            insert_unknown_value_import(
                                &specifier.exported_name,
                                &mut resolved_export_table.symbols,
                            );
                        }
                    }

                    continue;
                };

                ctx.set_file_name(parsed_file.file_name.clone());

                for specifier in specifiers {
                    let specifier_is_type_only = *is_type_only || specifier.is_type_only;
                    let type_export =
                        lookup_type_export(&target_export_table, &specifier.local_name);
                    let value_export =
                        lookup_value_export(&target_export_table, &specifier.local_name);

                    if specifier_is_type_only {
                        if let Some(type_export) = type_export {
                            export_local_type_declaration(
                                type_export,
                                &specifier.exported_name,
                                None,
                                Arc::make_mut(&mut resolved_export_table.type_declarations),
                            );
                            continue;
                        }

                        emit_missing_export_diagnostic(
                            ctx,
                            module_specifier,
                            &specifier.local_name,
                            specifier.name_span,
                        );
                        insert_unknown_type_import(
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        continue;
                    }

                    let mut found = false;

                    if let Some(type_export) = type_export {
                        export_local_type_declaration(
                            type_export,
                            &specifier.exported_name,
                            None,
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
                        );
                        found = true;
                    }

                    if let Some(value_export) = value_export {
                        if resolved_export_table
                            .symbols
                            .get(&specifier.exported_name)
                            .is_none()
                        {
                            let _ = resolved_export_table
                                .symbols
                                .insert_shared(specifier.exported_name.clone(), value_export);
                        }
                        found = true;
                    }

                    if !found {
                        if target_export_table.has_unresolved_star_export
                            || resolved_index
                                .map(|i| {
                                    module_has_unresolved_star_export(
                                        i,
                                        parsed_files,
                                        &ctx.module_file_index_by_identity,
                                    )
                                })
                                .unwrap_or(false)
                        {
                            insert_unknown_type_import(
                                Arc::make_mut(&mut resolved_export_table.type_declarations),
                                &specifier.exported_name,
                                ctx.file_name.clone(),
                                specifier.name_span,
                            );
                            if !specifier_is_type_only {
                                insert_unknown_value_import(
                                    &specifier.exported_name,
                                    &mut resolved_export_table.symbols,
                                );
                            }
                            continue;
                        }

                        if should_bind_unknown_for_missing_export(
                            &target_export_table,
                            resolved_index,
                            parsed_files,
                        ) {
                            insert_unknown_type_import(
                                Arc::make_mut(&mut resolved_export_table.type_declarations),
                                &specifier.exported_name,
                                ctx.file_name.clone(),
                                specifier.name_span,
                            );
                            if !specifier_is_type_only {
                                insert_unknown_value_import(
                                    &specifier.exported_name,
                                    &mut resolved_export_table.symbols,
                                );
                            }
                            continue;
                        }

                        emit_missing_export_diagnostic(
                            ctx,
                            module_specifier,
                            &specifier.local_name,
                            specifier.name_span,
                        );
                        insert_unknown_type_import(
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        if !specifier_is_type_only {
                            insert_unknown_value_import(
                                &specifier.exported_name,
                                &mut resolved_export_table.symbols,
                            );
                        }
                    }
                }
            }
            ParsedExportDeclaration::Namespace {
                exported_name,
                module_specifier,
                module_specifier_span,
                ..
            } => {
                let Some((target_export_table, _resolved_index)) = try_resolve_module_export_table(
                    module_specifier,
                    ctx,
                    parsed_files,
                    local_module_export_tables,
                    resolved_module_export_tables,
                    resolving,
                    &parsed_file.file_name,
                ) else {
                    if resolve_relative_module(
                        &parsed_file.file_name,
                        module_specifier,
                        parsed_files,
                        &ctx.module_file_index_by_identity,
                    )
                    .is_none()
                    {
                        record_unresolved_external_module(ctx, module_specifier);
                        if !(ctx.options.stub_external_modules
                            && is_external_specifier(module_specifier))
                        {
                            emit_unresolved_export_module_diagnostic(
                                ctx,
                                module_specifier,
                                *module_specifier_span,
                            );
                        }
                    }

                    insert_unknown_value_import(exported_name, &mut resolved_export_table.symbols);
                    continue;
                };

                ctx.set_file_name(parsed_file.file_name.clone());
                insert_namespace_export(
                    &mut resolved_export_table.symbols,
                    exported_name,
                    &target_export_table,
                );
                copy_namespace_member_type_exports(
                    &target_export_table,
                    exported_name,
                    &mut resolved_export_table,
                );
            }
            // `import * as z from "./m"; export { z }` re-exports the namespace
            // binding (zod's `z`). The value symbol is carried by the local
            // export pass; the namespace's TYPE side lives only in the importing
            // file's alias scope layers, so the qualified `z.<member>` keys must
            // be materialized into the export table here for the consumer-side
            // `copy_qualified_type_exports` to find.
            ParsedExportDeclaration::Named {
                specifiers,
                module_specifier: None,
                ..
            } => {
                for specifier in specifiers {
                    let Some(namespace_module_specifier) =
                        namespace_import_module_specifier(parsed_file, &specifier.local_name)
                    else {
                        continue;
                    };
                    let Some((target_export_table, _resolved_index)) =
                        try_resolve_module_export_table(
                            &namespace_module_specifier,
                            ctx,
                            parsed_files,
                            local_module_export_tables,
                            resolved_module_export_tables,
                            resolving,
                            &parsed_file.file_name,
                        )
                    else {
                        continue;
                    };
                    ctx.set_file_name(parsed_file.file_name.clone());
                    copy_namespace_member_type_exports(
                        &target_export_table,
                        &specifier.exported_name,
                        &mut resolved_export_table,
                    );
                }
            }
            _ => {}
        }
    }

    let re_export_start = Instant::now();
    for statement in &parsed_file.statements {
        let ParsedStatement::ExportDeclaration(export) = statement else {
            continue;
        };
        let ParsedExportDeclaration::All {
            module_specifier,
            module_specifier_span,
            ..
        } = export.as_ref()
        else {
            continue;
        };

        let Some((target_export_table, _resolved_index)) = try_resolve_module_export_table(
            module_specifier,
            ctx,
            parsed_files,
            local_module_export_tables,
            resolved_module_export_tables,
            resolving,
            &parsed_file.file_name,
        ) else {
            record_unresolved_external_module(ctx, module_specifier);
            if !(ctx.options.stub_external_modules && is_external_specifier(module_specifier)) {
                emit_unresolved_export_module_diagnostic(
                    ctx,
                    module_specifier,
                    *module_specifier_span,
                );
            }
            resolved_export_table.has_unresolved_star_export = true;
            continue;
        };

        ctx.set_file_name(parsed_file.file_name.clone());

        let resolved_type_declarations =
            Arc::make_mut(&mut resolved_export_table.type_declarations);
        for (name, declaration) in target_export_table.type_declarations.iter() {
            if resolved_type_declarations.get(name.as_ref()).is_none() {
                let _ = resolved_type_declarations.insert(name.clone(), declaration.clone());
            }
        }

        for (name, symbol) in target_export_table.symbols.iter_shared() {
            if resolved_export_table.symbols.get(name).is_none() {
                crate::program::record_module_export_symbol_handle_copy_count(1);
                resolved_export_table
                    .symbols
                    .insert_shared(name.clone(), symbol.clone());
            }
        }
    }
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.re_export_expansion += re_export_start.elapsed()
    });

    if let Some(slot) = resolving.get_mut(file_index) {
        *slot = false;
    }
    resolved_export_table.namespace_export_object_type =
        Some(compute_namespace_export_object_type(&resolved_export_table));
    if let Some(slot) = resolved_module_export_tables.get_mut(file_index) {
        *slot = Some(resolved_export_table.clone_with_reason(TypeCopyReason::ModuleExport));
    }
    Some(resolved_export_table)
}

pub(crate) fn collect_exportable_value_symbols(
    statements: &[ParsedStatement],
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> SymbolTable {
    let mut file_kinds = HashMap::new();
    file_kinds.insert(ctx.file_name.clone(), FileKind::RootSource);
    let mut shadow_ctx =
        CheckerContext::new(ctx.file_name.clone(), ctx.options.clone(), file_kinds);
    shadow_ctx.timings = ctx.timings.clone();

    let _ = local_type_declarations;
    shadow_ctx.type_declarations = ctx.type_declarations.clone();
    // The caller's full type-resolution surface must travel into the shadow, or
    // an exported `const` whose annotation names an *imported* type (a generic
    // arrow component's `ControllerProps<T, N>` parameter) resolves to `unknown`
    // and every consumer loses its signature. All Arc-shared, read-only state.
    // Library declaration files are exempt from the scope: they have no
    // initializers to infer and their exports resolve through the declaration
    // tables, while a live scope makes the `check_initializer` walk fully expand
    // library type graphs for every dependency module on every binding pass
    // (unnamed: 775MB/4.9s -> 8.5GB/66s peak RSS).
    if !ctx.is_library_scoped_file(&ctx.file_name) {
        shadow_ctx.type_declaration_scope = ctx.type_declaration_scope.clone();
    }
    shadow_ctx.ambient_global_type_declarations = ctx.ambient_global_type_declarations.clone();
    shadow_ctx.ambient_global_symbols = ctx
        .ambient_global_symbols
        .clone_with_reason(TypeCopyReason::ModuleExport);
    shadow_ctx.module_scope_by_file = ctx.module_scope_by_file.clone();
    shadow_ctx.module_local_values_by_file = ctx.module_local_values_by_file.clone();

    // The ambient globals (the lib `.d.ts` surface, ~1000 entries) are only a
    // read-only resolution backdrop here: the returned table is consulted via
    // `get`, never iterated, and the actual export entries are built into a fresh
    // table by the caller. Holding the globals as a `parent` fallback rather than
    // as the own map keeps each module's export-table build O(local symbols)
    // instead of deep-copying every global on the first local insert.
    let mut exportable_values = SymbolTable::new();
    for (name, symbol) in local_symbols.iter_shared() {
        let _ = exportable_values.insert_shared(name.clone(), symbol.clone());
    }
    let mut exportable_values = exportable_values.with_parent_fallback(Arc::new(
        ctx.ambient_global_symbols
            .clone_with_reason(TypeCopyReason::ModuleExport),
    ));

    for statement in statements {
        collect_exportable_value_symbols_from_statement(
            statement,
            &mut exportable_values,
            &mut shadow_ctx,
        );
    }

    exportable_values
}

pub(crate) fn collect_exportable_value_symbols_from_statement(
    statement: &ParsedStatement,
    exportable_values: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            let existing_symbol = exportable_values.get_shared(&variable.name);
            let _ = check_variable_declaration_with_symbols(
                variable.as_ref().clone(),
                exportable_values,
                ctx,
                VariableCheckOptions {
                    report_duplicate_let_const: false,
                    check_initializer: true,
                },
            );

            if let Some(existing_symbol) = existing_symbol {
                exportable_values.insert_shared(variable.name.clone(), existing_symbol);
            }
        }
        ParsedStatement::ExportDeclaration(export) => {
            if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                collect_exportable_value_symbols_from_statement(
                    declaration.as_ref(),
                    exportable_values,
                    ctx,
                )
            }
        }
        ParsedStatement::NamespaceDeclaration(namespace) => {
            if exportable_values.get(&namespace.name).is_none() {
                let _ = exportable_values.insert(
                    namespace.name.clone(),
                    SymbolInfo {
                        ty: namespace_value_object_type(namespace),
                        kind: SymbolKind::Const,
                        function_signature: None,
                    },
                );
            }
        }
        _ => {}
    }
}

/// The value-side object type of a `declare namespace`: one property per value
/// member (functions, consts, classes, nested namespaces). Member types are kept
/// permissive (functions accept any arguments, everything else is `any`) so the
/// namespace's member *set* is precise — enabling TS2339 on real typos — without
/// re-resolving a partially modelled surface and cascading. Used to bind an
/// `export = <namespace>` value so `import * as Ns` exposes `Ns.member`.
pub(crate) fn namespace_value_object_type(namespace: &ParsedNamespaceDeclaration) -> Type {
    let mut properties = surge_ts_types::PropertyMap::new();
    fill_namespace_value_properties(namespace, &mut properties);
    Type::Object(crate::arena::alloc_object_type(properties, None))
}

/// Accumulate a `declare namespace`'s value members into `properties`. Split into
/// its own function so a namespace declared across multiple merged blocks (e.g.
/// roblox-ts's `math`, declared with `noise`/`clamp` in one file and the Lua math
/// surface in another) can be assembled into a single value object.
pub(crate) fn fill_namespace_value_properties(
    namespace: &ParsedNamespaceDeclaration,
    properties: &mut surge_ts_types::PropertyMap,
) {
    use surge_ts_types::{FunctionType, ObjectProperty};

    for statement in &namespace.statements {
        let inner = match statement {
            ParsedStatement::ExportDeclaration(export) => {
                if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                    declaration.as_ref()
                } else {
                    statement
                }
            }
            other => other,
        };

        match inner {
            ParsedStatement::FunctionDeclaration(function) => {
                properties.insert(
                    function.name.clone(),
                    ObjectProperty::required(Type::Function(FunctionType::new(
                        vec![],
                        Type::Any,
                        true,
                        0,
                    ))),
                );
            }
            ParsedStatement::VariableDeclaration(variable) => {
                properties.insert(variable.name.clone(), ObjectProperty::required(Type::Any));
            }
            ParsedStatement::ClassDeclaration(class) => {
                properties.insert(class.name.clone(), ObjectProperty::required(Type::Any));
            }
            ParsedStatement::NamespaceDeclaration(inner_namespace) => {
                properties.insert(
                    inner_namespace.name.clone(),
                    ObjectProperty::required(namespace_value_object_type(inner_namespace)),
                );
            }
            _ => {}
        }
    }
}

pub(crate) fn collect_exports_from_statement(
    statement: &ParsedStatement,
    exportable_values: &SymbolTable,
    imported_symbols: &SymbolTable,
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    default_symbol: &mut Option<Arc<SymbolInfo>>,
    export_assignment_symbol: &mut Option<Arc<SymbolInfo>>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                collect_exports_from_statement(
                    declaration.as_ref(),
                    exportable_values,
                    imported_symbols,
                    local_type_declarations,
                    local_symbols,
                    resolution_scope,
                    type_declarations,
                    symbols,
                    default_symbol,
                    export_assignment_symbol,
                    ctx,
                )
            }
            ParsedExportDeclaration::Equals { exported_name, .. } => {
                // `export = identifier` binds the module's single export-assignment
                // value to the named local value symbol. An unresolved target binds
                // nothing and emits no diagnostic here, leaving consumers to bind an
                // unknown placeholder rather than cascade (`import x = require(...)`).
                if let Some(symbol) = exportable_values.get_shared(exported_name) {
                    *export_assignment_symbol = Some(symbol);
                }

                // When the export target is a `declare namespace <name>`, its type
                // members were collected under `<name>.<member>` keys. Carry them into
                // the export table so a namespace import (`import * as React`) can
                // re-expose them as qualified types (`React.ComponentProps<...>`).
                let prefix = format!("{exported_name}.");
                for (key, declaration) in local_type_declarations.iter() {
                    if key.as_str().starts_with(&prefix) {
                        let _ = type_declarations.insert(key.as_str(), declaration.clone());
                        let exported_member_name = &key.as_str()[prefix.len()..];
                        let _ = type_declarations.insert(
                            exported_member_name,
                            rename_type_declaration(
                                attach_type_resolution_scope_if_missing(
                                    declaration.clone(),
                                    resolution_scope,
                                ),
                                exported_member_name.to_string(),
                            ),
                        );
                    }
                }
            }
            ParsedExportDeclaration::Named {
                is_type_only,
                specifiers,
                module_specifier,
                ..
            } => {
                if module_specifier.is_some() {
                    return;
                }

                for specifier in specifiers {
                    let specifier_is_type_only = *is_type_only || specifier.is_type_only;

                    if specifier_is_type_only {
                        export_local_type_name(
                            &specifier.local_name,
                            &specifier.exported_name,
                            &specifier.name_span,
                            local_type_declarations,
                            resolution_scope,
                            type_declarations,
                            ctx,
                        );
                        continue;
                    }

                    let mut found = false;

                    if let Some(type_declaration) =
                        local_type_declarations.get(&specifier.local_name)
                    {
                        export_local_type_declaration(
                            type_declaration,
                            &specifier.exported_name,
                            resolution_scope,
                            type_declarations,
                        );
                        found = true;
                    }

                    if let Some(symbol) = exportable_values
                        .get_shared(&specifier.local_name)
                        .or_else(|| imported_symbols.get_shared(&specifier.local_name))
                    {
                        if symbols.get(&specifier.exported_name).is_none() {
                            symbols.insert_shared(specifier.exported_name.clone(), symbol);
                        }
                        found = true;
                    }

                    if !found {
                        push_unresolved_export_diagnostic(
                            ctx,
                            &specifier.local_name,
                            specifier.name_span,
                        );
                    }
                }
            }
            ParsedExportDeclaration::Default { declaration, span } => match declaration {
                ParsedDefaultExportDeclaration::Function(function) => {
                    if default_symbol.is_some() {
                        push_duplicate_default_export_diagnostic(ctx, function.name_span.or(*span));
                    } else {
                        let mut signature_symbols =
                            exportable_values.clone_with_reason(TypeCopyReason::ModuleExport);
                        let mut function_type =
                            check_function::collect_function_declaration_signature(
                                function,
                                &mut signature_symbols,
                                ctx,
                            );
                        if let Some(value_type) =
                            promise_value_type(&function.return_type, resolution_scope, ctx)
                        {
                            function_type = FunctionType::new(
                                function_type.parameters().to_vec(),
                                promise_like_type(value_type),
                                function_type.is_variadic(),
                                function_type.required_parameter_count(),
                            );
                        }
                        *default_symbol = Some(Arc::new(SymbolInfo {
                            ty: Type::Function(function_type),
                            kind: SymbolKind::Function,
                            function_signature: None,
                        }));
                    }
                }
                ParsedDefaultExportDeclaration::Class(class) => {
                    if let Some(symbol) = local_symbols.get_shared(&class.name) {
                        if default_symbol.is_some() {
                            push_duplicate_default_export_diagnostic(
                                ctx,
                                class.name_span.or(*span),
                            );
                        } else {
                            *default_symbol = Some(symbol);
                        }
                    } else {
                        push_duplicate_default_export_diagnostic(ctx, class.name_span.or(*span));
                    }
                }
                ParsedDefaultExportDeclaration::Expression(expression) => {
                    if default_symbol.is_some() {
                        push_duplicate_default_export_diagnostic(ctx, *span);
                        return;
                    }

                    let ty = crate::infer::infer_expression(expression, exportable_values, ctx);
                    let ty = match ty {
                        crate::infer::InferredExpression::Known(ty) => ty,
                        crate::infer::InferredExpression::Unknown
                        | crate::infer::InferredExpression::UnresolvedIdentifier { .. }
                        | crate::infer::InferredExpression::MissingProperty { .. } => Type::Unknown,
                    };

                    *default_symbol = Some(Arc::new(SymbolInfo {
                        ty,
                        kind: SymbolKind::Const,
                        function_signature: None,
                    }));
                }
                ParsedDefaultExportDeclaration::Unsupported { .. } => {}
            },
            ParsedExportDeclaration::Namespace { .. } => {}
            ParsedExportDeclaration::All { .. } => {}
            _ => {}
        },
        ParsedStatement::TypeAliasDeclaration(alias) => {
            export_local_type_name(
                &alias.name,
                &alias.name,
                &alias.name_span,
                local_type_declarations,
                resolution_scope,
                type_declarations,
                ctx,
            );
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            export_local_type_name(
                &interface.name,
                &interface.name,
                &interface.name_span,
                local_type_declarations,
                resolution_scope,
                type_declarations,
                ctx,
            );
        }
        ParsedStatement::FunctionDeclaration(function) => {
            if let Some(symbol) = local_symbols.get_shared(&function.name) {
                if symbols.get(&function.name).is_none() {
                    symbols.insert_shared(function.name.clone(), symbol);
                }
            }
        }
        ParsedStatement::ClassDeclaration(class) => {
            // A class exports both an instance type and a constructor/static value.
            if let Some(type_declaration) = local_type_declarations.get(&class.name) {
                export_local_type_declaration(
                    type_declaration,
                    &class.name,
                    resolution_scope,
                    type_declarations,
                );
            }
            if let Some(symbol) = local_symbols.get_shared(&class.name) {
                if symbols.get(&class.name).is_none() {
                    symbols.insert_shared(class.name.clone(), symbol);
                }
            }
        }
        ParsedStatement::VariableDeclaration(variable) => {
            if let Some(symbol) = exportable_values.get_shared(&variable.name) {
                if symbols.get(&variable.name).is_none() {
                    symbols.insert_shared(variable.name.clone(), symbol);
                }
            }
        }
        ParsedStatement::NamespaceDeclaration(namespace) => {
            // `export namespace ns { … }` exports the namespace's value object
            // and its type members. The members were collected under qualified
            // `ns.<member>` keys (same shape the `export =` path consumes), so
            // carry those keys into the export table for qualified references
            // (`ns.Member`) on the importing side.
            if let Some(symbol) = exportable_values.get_shared(&namespace.name) {
                if symbols.get(&namespace.name).is_none() {
                    symbols.insert_shared(namespace.name.clone(), symbol);
                }
            }

            if let Some(type_declaration) = local_type_declarations.get(&namespace.name) {
                export_local_type_declaration(
                    type_declaration,
                    &namespace.name,
                    resolution_scope,
                    type_declarations,
                );
            }

            let prefix = format!("{}.", namespace.name);
            for (key, declaration) in local_type_declarations.iter() {
                if key.as_str().starts_with(&prefix) && type_declarations.get(key.as_str()).is_none()
                {
                    let _ = type_declarations.insert(
                        key.as_str(),
                        attach_type_resolution_scope_if_missing(
                            declaration.clone(),
                            resolution_scope,
                        ),
                    );
                }
            }
        }
        _ => {}
    }
}

pub(crate) const PROMISE_LIKE_VALUE_PROPERTY: &str = "\0surgePromiseValue";

pub(crate) fn promise_like_type(value_type: Type) -> Type {
    let mut properties = PropertyMap::new();
    properties.insert(
        PROMISE_LIKE_VALUE_PROPERTY.to_string(),
        ObjectProperty::required(value_type.clone()),
    );
    properties.insert(
        "then".to_string(),
        ObjectProperty::required(Type::Function(FunctionType::new(
            vec![Type::Function(FunctionType::new(
                vec![value_type],
                Type::Unknown,
                false,
                1,
            ))],
            Type::Unknown,
            true,
            1,
        ))),
    );
    properties.insert(
        "catch".to_string(),
        ObjectProperty::required(Type::Function(FunctionType::new(
            vec![Type::Any],
            Type::Unknown,
            true,
            0,
        ))),
    );
    properties.insert(
        "finally".to_string(),
        ObjectProperty::required(Type::Function(FunctionType::new(
            vec![Type::Any],
            Type::Unknown,
            true,
            0,
        ))),
    );
    Type::Object(crate::arena::alloc_object_type(properties, None))
}

fn promise_value_type(
    return_type: &Option<ParsedType>,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(ParsedType::Named(named)) = return_type else {
        return None;
    };
    if !matches!(named.name.as_str(), "Promise" | "PromiseLike") {
        return None;
    }
    let value_type = named.type_arguments.first()?;
    let saved_scope = ctx.type_declaration_scope.clone();
    let saved_type_declarations = if resolution_scope.is_some() {
        Some(std::mem::replace(
            &mut ctx.type_declarations,
            TypeDeclarationTable::new(),
        ))
    } else {
        None
    };
    if let Some(resolution_scope) = resolution_scope {
        ctx.type_declaration_scope = Some(resolution_scope.clone());
    }
    let ty = crate::infer::map_parsed_type(value_type.clone(), ctx).peeled();
    ctx.type_declaration_scope = saved_scope;
    if let Some(saved_type_declarations) = saved_type_declarations {
        ctx.type_declarations = saved_type_declarations;
    }
    Some(ty)
}

pub(crate) fn export_local_type_name(
    local_name: &str,
    exported_name: &str,
    name_span: &Option<TextSpan>,
    local_type_declarations: &TypeDeclarationTable,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
    ctx: &mut CheckerContext,
) {
    // Read the local declaration through an arena-backed handle so re-export
    // binding hands `export_local_type_declaration` a borrow instead of a deep
    // clone. The rename/scope rewrite there still takes one owned copy; this
    // removes the redundant second clone this path previously paid per
    // re-exported type.
    let Some(handle) = local_type_declarations.get_handle(local_name) else {
        push_unresolved_export_diagnostic(ctx, local_name, *name_span);
        return;
    };

    export_local_type_declaration(
        handle.get(),
        exported_name,
        resolution_scope,
        type_declarations,
    );
}

pub(crate) fn export_local_type_declaration(
    declaration: &TypeDeclarationInfo,
    exported_name: &str,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
) {
    let declaration = rename_type_declaration(
        attach_type_resolution_scope_if_missing(declaration.clone(), resolution_scope),
        exported_name.to_string(),
    );
    let _ = type_declarations.insert(exported_name.to_string(), declaration);
}

pub(crate) fn rename_type_declaration(
    declaration: TypeDeclarationInfo,
    exported_name: String,
) -> TypeDeclarationInfo {
    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            if alias.declared_name.is_none() {
                alias.declared_name = Some(alias.name.clone());
            }
            alias.name = exported_name;
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            if interface.declared_name.is_none() {
                interface.declared_name = Some(interface.name.clone());
            }
            interface.name = exported_name;
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

pub(crate) fn insert_type_export(
    type_declarations: &mut TypeDeclarationTable,
    local_name: &str,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    declaration: TypeDeclarationInfo,
) {
    let declaration = rename_type_declaration(
        attach_type_resolution_scope_if_missing(declaration, resolution_scope),
        local_name.to_string(),
    );
    let _ = type_declarations.insert(local_name.to_string(), declaration);
}

pub(crate) fn insert_unknown_type_import(
    type_declarations: &mut TypeDeclarationTable,
    local_name: &str,
    file_name: String,
    name_span: Option<TextSpan>,
) {
    let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
        local_name.to_string(),
        file_name,
        name_span,
        vec![],
        ParsedType::Unknown,
        None,
    ));

    let _ = type_declarations.insert(local_name.to_string(), declaration);
}

pub(crate) fn insert_unknown_value_import(local_name: &str, symbols: &mut SymbolTable) {
    let _ = symbols.insert(
        local_name.to_string(),
        SymbolInfo {
            ty: Type::Unknown,
            kind: SymbolKind::Var,
            function_signature: None,
        },
    );
}

pub(crate) fn lookup_type_export<'a>(
    export_table: &'a ModuleExportTable,
    local_name: &'a str,
) -> Option<&'a TypeDeclarationInfo> {
    crate::program::record_module_export_borrowed_lookup_count();
    export_table.type_declarations.get(local_name)
}

/// Copy an exported namespace's qualified type members (`ns.Member`) into the
/// importer's scope under the local binding name (`local.Member`), so qualified
/// type references through a named namespace import resolve.
/// Copies every `<imported_name>.<member>` qualified type export under the
/// local binding name. Returns whether any member was copied — a `true` means
/// the imported name exists as a (type-only) namespace even when it has no
/// direct type/value export entry of its own.
pub(crate) fn copy_qualified_type_exports(
    export_table: &ModuleExportTable,
    imported_name: &str,
    local_name: &str,
    type_declarations: &mut TypeDeclarationTable,
) -> bool {
    let prefix = format!("{imported_name}.");
    let mut copied_any = false;
    for (key, declaration) in export_table.type_declarations.iter() {
        if let Some(member) = key.as_str().strip_prefix(&prefix) {
            copied_any = true;
            let local_key = format!("{local_name}.{member}");
            if type_declarations.get(&local_key).is_none() {
                let _ = type_declarations.insert(local_key.as_str(), declaration.clone());
            }
        }
    }
    copied_any
}

pub(crate) fn lookup_value_export(
    export_table: &ModuleExportTable,
    local_name: &str,
) -> Option<Arc<SymbolInfo>> {
    if local_name == "default" {
        return export_table.get_shared_value("default");
    }

    export_table.get_shared_value(local_name)
}

/// The module specifier of a `import * as <local_name>` declaration in this
/// file, if any — the binding a `export { <local_name> }` re-export refers to.
fn namespace_import_module_specifier(
    parsed_file: &ParsedProgramFile,
    local_name: &str,
) -> Option<String> {
    parsed_file.statements.iter().find_map(|statement| {
        let ParsedStatement::ImportDeclaration(import) = statement else {
            return None;
        };
        match &import.kind {
            ParsedImportKind::Namespace {
                local_name: import_local_name,
                ..
            } if import_local_name == local_name => Some(import.module_specifier.clone()),
            _ => None,
        }
    })
}

/// Materializes a re-exported namespace's type members into the export table as
/// qualified `<exported_name>.<member>` keys, mirroring how consumers of
/// `import * as` see them through alias scope layers.
fn copy_namespace_member_type_exports(
    target_export_table: &ModuleExportTable,
    exported_name: &str,
    resolved_export_table: &mut ModuleExportTable,
) {
    let type_declarations = Arc::make_mut(&mut resolved_export_table.type_declarations);
    for (key, declaration) in target_export_table.type_declarations.iter() {
        let qualified = format!("{exported_name}.{key}");
        if type_declarations.get(&qualified).is_none() {
            let _ = type_declarations.insert(qualified.as_str(), declaration.clone());
        }
    }
}

pub(crate) fn insert_namespace_export(
    symbols: &mut SymbolTable,
    exported_name: &str,
    export_table: &ModuleExportTable,
) {
    let _ = symbols.insert(
        exported_name.to_string(),
        SymbolInfo {
            ty: namespace_export_object_type(export_table),
            kind: SymbolKind::Const,
            function_signature: None,
        },
    );
}

pub(crate) fn namespace_export_object_type(export_table: &ModuleExportTable) -> Type {
    if let Some(namespace_export_object_type) = &export_table.namespace_export_object_type {
        crate::program::record_module_export_borrowed_lookup_count();
        return namespace_export_object_type.clone();
    }

    compute_namespace_export_object_type(export_table)
}

pub(crate) fn compute_namespace_export_object_type(export_table: &ModuleExportTable) -> Type {
    crate::program::record_module_export_namespace_export_object_materialization_count();
    let mut properties = surge_ts_types::PropertyMap::new();
    let mut property_count = 0u64;

    for (name, symbol) in export_table.symbols.iter() {
        property_count += 1;
        properties.insert(
            name.to_string(),
            surge_ts_types::ObjectProperty::required(symbol.ty.clone()),
        );
    }

    if let Some(default_symbol) = &export_table.default_symbol {
        property_count += 1;
        properties.insert(
            "default".to_string(),
            surge_ts_types::ObjectProperty::required(default_symbol.ty.clone()),
        );
    }

    // `export = <namespace>` exposes the namespace object as the module's shape;
    // surface its members (e.g. `React.createContext`) on the namespace import.
    if let Some(export_assignment_symbol) = &export_table.export_assignment_symbol {
        if let Type::Object(object) = &export_assignment_symbol.ty {
            for (name, property) in object.properties.iter() {
                property_count += 1;
                properties
                    .entry(name.clone())
                    .or_insert_with(|| property.clone());
            }
        }
    }

    crate::program::record_module_export_namespace_export_object_property_count(property_count);

    Type::Object(crate::arena::alloc_object_type(properties, None))
}

pub(crate) fn attach_type_resolution_scope(
    declaration: TypeDeclarationInfo,
    resolution_scope: Arc<TypeDeclarationScope>,
) -> TypeDeclarationInfo {
    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            if alias.resolution_scope.is_none() {
                alias.resolution_scope = Some(resolution_scope);
            }
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            if interface.resolution_scope.is_none() {
                interface.resolution_scope = Some(resolution_scope);
            }
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

pub(crate) fn attach_type_resolution_scope_if_missing(
    declaration: TypeDeclarationInfo,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
) -> TypeDeclarationInfo {
    match resolution_scope {
        Some(scope) => attach_type_resolution_scope(declaration, scope.clone()),
        None => declaration,
    }
}
