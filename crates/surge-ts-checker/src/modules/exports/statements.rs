use super::*;
use surge_ts_syntax::ParsedExpression;

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
                // The value twin of the qualified type carry below: a namespace
                // member published under `<name>.<member>` holds the member's
                // real signature, while the namespace object only models the
                // member *set* with permissive `any`. Without this, every
                // `import { useState } from "react"` bound React's permissive
                // member and lost the hook's return type.
                for (key, symbol) in exportable_values.iter_shared() {
                    let Some(member_name) = key.strip_prefix(prefix.as_str()) else {
                        continue;
                    };
                    if symbols.get_own_shared(key.as_ref()).is_none() {
                        symbols.insert_shared(key.to_string(), symbol.clone());
                    }
                    if symbols.get_own_shared(member_name).is_none() {
                        symbols.insert_shared(member_name.to_string(), symbol.clone());
                    }
                }
                for (key, declaration) in local_type_declarations.iter() {
                    if key.starts_with(&prefix) {
                        let _ = type_declarations.insert(key.as_ref(), declaration.clone());
                        let exported_member_name = &key[prefix.len()..];
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

                    // Imported types are not in `local_type_declarations`; the
                    // resolution scope's import layers carry them. Local-first
                    // layer order keeps the own-declaration case unchanged.
                    if let Some(handle) = local_type_declarations
                        .get_handle(&specifier.local_name)
                        .or_else(|| {
                            resolution_scope
                                .and_then(|scope| scope.get_handle(&specifier.local_name))
                        })
                    {
                        export_local_type_declaration(
                            handle.get(),
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
                        if specifier.exported_name == "default" {
                            // `export { x as default }` — consumers read the
                            // default through `default_symbol`, never `symbols`.
                            if default_symbol.is_none() {
                                *default_symbol = Some(symbol);
                            }
                        } else if symbols.get(&specifier.exported_name).is_none() {
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
                                false,
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
                    publish_default_type_export(
                        &class.name,
                        local_type_declarations,
                        resolution_scope,
                        type_declarations,
                    );
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

                    // `export default Dispatcher` re-exports the named
                    // declaration's type side too, not just its value.
                    if let ParsedExpression::Identifier { name, .. } = expression {
                        publish_default_type_export(
                            name,
                            local_type_declarations,
                            resolution_scope,
                            type_declarations,
                        );
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
                // The expression form is not modelled, but the module
                // demonstrably HAS a default export; recording none would make
                // every consumer report a missing `default` member.
                ParsedDefaultExportDeclaration::Unsupported { .. } => {
                    if default_symbol.is_none() {
                        *default_symbol = Some(Arc::new(SymbolInfo {
                            ty: Type::Unknown,
                            kind: SymbolKind::Const,
                            function_signature: None,
                        }));
                    }
                }
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

            // The value members are collected under the same qualified
            // `ns.<member>` keys; carry them over so a call through the
            // namespace on the importing side finds the member's real signature.
            let value_prefix = format!("{}.", namespace.name);
            for (key, symbol) in exportable_values.iter_shared() {
                if key.starts_with(&value_prefix) && symbols.get(key.as_ref()).is_none() {
                    symbols.insert_shared(key.as_ref().to_string(), symbol.clone());
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
                if key.starts_with(&prefix) && type_declarations.get(key.as_ref()).is_none() {
                    let _ = type_declarations.insert(
                        key.as_ref(),
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

/// Publishes `local_name`'s type declaration under the `default` export key.
///
/// A default-exported class (or an identifier naming one) contributes a type as
/// well as a value, and a consumer's `import D from "./m"` binds both. The name
/// may itself be imported here (`import D from "./d"; export default D`), which
/// puts it in the resolution scope's import layers rather than this file's own
/// declaration table.
fn publish_default_type_export(
    local_name: &str,
    local_type_declarations: &TypeDeclarationTable,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
) {
    let Some(handle) = local_type_declarations
        .get_handle(local_name)
        .or_else(|| resolution_scope.and_then(|scope| scope.get_handle(local_name)))
    else {
        return;
    };

    if type_declarations.get("default").is_some() {
        return;
    }

    export_local_type_declaration(handle.get(), "default", resolution_scope, type_declarations);
}
