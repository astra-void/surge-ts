//! Building and resolving per-module export tables (named, default, star, namespace).

use super::*;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use surge_ts_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedNamespaceDeclaration,
    ParsedStatement, ParsedType, TextSpan,
};
use surge_ts_types::{Type, TypeCopyReason};

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
        match statement {
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
                is_type_only,
                specifiers,
                module_specifier: Some(module_specifier),
                module_specifier_span,
                ..
            }) => {
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
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Namespace {
                exported_name,
                module_specifier,
                module_specifier_span,
                ..
            }) => {
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
            }
            _ => {}
        }
    }

    let re_export_start = Instant::now();
    for statement in &parsed_file.statements {
        let ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
            module_specifier,
            module_specifier_span,
            ..
        }) = statement
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

        let resolved_type_declarations = Arc::make_mut(&mut resolved_export_table.type_declarations);
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
                variable.clone(),
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
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_exportable_value_symbols_from_statement(
            declaration.as_ref(),
            exportable_values,
            ctx,
        ),
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
fn namespace_value_object_type(namespace: &ParsedNamespaceDeclaration) -> Type {
    use surge_ts_types::{FunctionType, ObjectProperty};

    let mut properties = surge_ts_types::PropertyMap::new();
    for statement in &namespace.statements {
        let inner = match statement {
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
                declaration,
                ..
            }) => declaration.as_ref(),
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

    Type::Object(crate::arena::alloc_object_type(properties, None))
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
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_exports_from_statement(
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
        ),
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Equals {
            exported_name,
            ..
        }) => {
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
                }
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
            is_type_only,
            specifiers,
            module_specifier,
            ..
        }) => {
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

                if let Some(type_declaration) = local_type_declarations.get(&specifier.local_name) {
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
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration,
            span,
        }) => match declaration {
            ParsedDefaultExportDeclaration::Function(function) => {
                if let Some(symbol) = local_symbols.get_shared(&function.name) {
                    if default_symbol.is_some() {
                        push_duplicate_default_export_diagnostic(ctx, function.name_span.or(*span));
                    } else {
                        *default_symbol = Some(symbol);
                    }
                } else {
                    push_duplicate_default_export_diagnostic(ctx, function.name_span.or(*span));
                }
            }
            ParsedDefaultExportDeclaration::Class(class) => {
                if let Some(symbol) = local_symbols.get_shared(&class.name) {
                    if default_symbol.is_some() {
                        push_duplicate_default_export_diagnostic(ctx, class.name_span.or(*span));
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
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Namespace { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All { .. }) => {}
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
        _ => {}
    }
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

pub(crate) fn lookup_value_export(
    export_table: &ModuleExportTable,
    local_name: &str,
) -> Option<Arc<SymbolInfo>> {
    if local_name == "default" {
        return export_table.get_shared_value("default");
    }

    export_table.get_shared_value(local_name)
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
