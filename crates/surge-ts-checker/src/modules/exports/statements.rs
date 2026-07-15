use super::*;

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
