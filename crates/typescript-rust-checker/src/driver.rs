use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedInterfaceDeclaration,
    ParsedStatement, ParsedTypeAliasDeclaration, parse_source,
};

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::CheckerContext;
use crate::infer::report_duplicate_type_parameters;
use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

pub fn check_source(source_text: &str, file_name: &str) -> Vec<Diagnostic> {
    check_source_with_options(
        source_text,
        file_name,
        crate::context::CheckerOptions::default(),
    )
}

pub fn check_source_with_options(
    source_text: &str,
    file_name: &str,
    options: crate::context::CheckerOptions,
) -> Vec<Diagnostic> {
    let parsed = parse_source(source_text, file_name);
    let file_name = parsed.file_name;
    let mut ctx = CheckerContext::new(file_name.clone(), options);

    crate::builtins::inject_builtins(&mut ctx);

    for message in parsed.parser_errors {
        let diagnostic = Diagnostic::typescript_rust_parser_error(message, file_name.clone());
        ctx.push(diagnostic);
    }

    collect_type_declarations(&parsed.statements, &mut ctx);

    for statement in parsed.statements {
        check_statement(statement, &mut ctx);
    }

    ctx.finish()
}

pub(crate) fn collect_type_declarations(statements: &[ParsedStatement], ctx: &mut CheckerContext) {
    for statement in statements {
        collect_type_declarations_from_statement(statement, ctx);
    }
}

fn collect_type_declarations_from_statement(statement: &ParsedStatement, ctx: &mut CheckerContext) {
    match statement {
        ParsedStatement::TypeAliasDeclaration(alias) => {
            collect_type_alias(alias, ctx);
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            collect_interface(interface, ctx);
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_type_declarations_from_statement(declaration.as_ref(), ctx),
        _ => {}
    }
}

fn check_statement(statement: ParsedStatement, ctx: &mut CheckerContext) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            var::check_variable_declaration(variable, ctx);
        }
        ParsedStatement::Assignment(assignment) => {
            assign::check_assignment(assignment, ctx);
        }
        ParsedStatement::FunctionDeclaration(function) => {
            check_function::check_function_declaration(function, ctx);
        }
        ParsedStatement::Call(call) => {
            call::check_call(call, ctx);
        }
        ParsedStatement::Expression(expression) => {
            expr::check_expression_statement(expression, ctx);
        }
        ParsedStatement::TypeAliasDeclaration(_) => {}
        ParsedStatement::InterfaceDeclaration(_) => {}
        ParsedStatement::ImportDeclaration(import) => {
            if crate::modules::is_external_specifier(&import.module_specifier) {
                if !ctx.options.stub_external_modules {
                    let mut diagnostic = match &import.kind {
                        typescript_rust_syntax::ParsedImportKind::SideEffect => {
                            Diagnostic::ts2882(&import.module_specifier, ctx.file_name.clone())
                        }
                        _ => Diagnostic::ts2307(&import.module_specifier, ctx.file_name.clone()),
                    };
                    if let Some(span) = import.module_specifier_span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }
                    ctx.push(diagnostic);
                }

                // Stub the imports to avoid cascades in single-file mode
                match &import.kind {
                    typescript_rust_syntax::ParsedImportKind::Named {
                        specifiers,
                        is_type_only,
                    } => {
                        for specifier in specifiers {
                            if *is_type_only {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo {
                                        name: specifier.local_name.to_string(),
                                        file_name: ctx.file_name.clone(),
                                        name_span: specifier.name_span,
                                        type_parameters: vec![],
                                        ty: typescript_rust_syntax::ParsedType::Unknown,
                                        resolution_scope: None,
                                    },
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                            } else {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo {
                                        name: specifier.local_name.to_string(),
                                        file_name: ctx.file_name.clone(),
                                        name_span: specifier.name_span,
                                        type_parameters: vec![],
                                        ty: typescript_rust_syntax::ParsedType::Unknown,
                                        resolution_scope: None,
                                    },
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                                let _ = ctx.symbols.insert(
                                    specifier.local_name.clone(),
                                    crate::symbols::SymbolInfo {
                                        ty: typescript_rust_types::Type::Unknown,
                                        kind: crate::symbols::SymbolKind::Var,
                                    },
                                );
                            }
                        }
                    }
                    typescript_rust_syntax::ParsedImportKind::Default { local_name, .. } => {
                        let _ = ctx.symbols.insert(
                            local_name.clone(),
                            crate::symbols::SymbolInfo {
                                ty: typescript_rust_types::Type::Unknown,
                                kind: crate::symbols::SymbolKind::Var,
                            },
                        );
                    }
                    typescript_rust_syntax::ParsedImportKind::Namespace { local_name, .. } => {
                        let _ = ctx.symbols.insert(
                            local_name.clone(),
                            crate::symbols::SymbolInfo {
                                ty: typescript_rust_types::Type::Unknown,
                                kind: crate::symbols::SymbolKind::Const,
                            },
                        );
                    }
                    _ => {}
                }
            } else {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = import.span.or(import.module_specifier_span) {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => check_statement(*declaration, ctx),
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
            module_specifier: Some(specifier),
            span,
            module_specifier_span,
            ..
        }) => {
            if crate::modules::is_external_specifier(&specifier) {
                if !ctx.options.stub_external_modules {
                    let mut diagnostic = Diagnostic::ts2307(&specifier, ctx.file_name.clone());
                    if let Some(span) = module_specifier_span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }
                    ctx.push(diagnostic);
                }
            } else {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration,
            span,
        }) => match declaration {
            ParsedDefaultExportDeclaration::Function(function) => {
                check_function::check_function_declaration(function, ctx);
            }
            ParsedDefaultExportDeclaration::Expression(expression) => {
                expr::check_expression_statement(expression, ctx);
            }
            ParsedDefaultExportDeclaration::Unsupported { .. } => {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        },
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
            module_specifier,
            span,
            module_specifier_span,
            ..
        }) => {
            if crate::modules::is_external_specifier(&module_specifier) {
                if !ctx.options.stub_external_modules {
                    let mut diagnostic =
                        Diagnostic::ts2307(&module_specifier, ctx.file_name.clone());
                    if let Some(span) = module_specifier_span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }
                    ctx.push(diagnostic);
                }
            } else {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Empty { .. }) => {}
        ParsedStatement::DeclareModuleDeclaration(_) => {}
        ParsedStatement::UnsupportedDeclaration { span } => {
            let mut diag =
                typescript_rust_diagnostics::Diagnostic::typescript_rust_unsupported_declaration(
                    ctx.file_name.clone(),
                );
            if let Some(s) = span {
                diag = diag.with_span(crate::context::convert_span(s));
            }
            ctx.push(diag);
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { span }) => {
            let mut diagnostic =
                Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

            if let Some(span) = span {
                diagnostic = diagnostic.with_span(crate::context::convert_span(span));
            }

            ctx.push(diagnostic);
        }
    }
}

pub(crate) fn collect_type_alias(alias: &ParsedTypeAliasDeclaration, ctx: &mut CheckerContext) {
    report_duplicate_type_parameters(&alias.type_parameters, ctx);

    let info = TypeAliasInfo {
        name: alias.name.clone(),
        file_name: ctx.file_name.clone(),
        name_span: alias.name_span,
        type_parameters: alias.type_parameters.clone(),
        ty: alias.ty.clone(),
        resolution_scope: None,
    };

    if ctx
        .type_declarations
        .insert(alias.name.clone(), TypeDeclarationInfo::Alias(info))
        .is_some()
    {
        let mut diagnostic = Diagnostic::ts2300(&alias.name, ctx.file_name.clone());

        if let Some(span) = alias.name_span {
            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
        }

        ctx.push(diagnostic);
    }
}

pub(crate) fn collect_interface(interface: &ParsedInterfaceDeclaration, ctx: &mut CheckerContext) {
    report_duplicate_type_parameters(&interface.type_parameters, ctx);

    let info = InterfaceInfo {
        name: interface.name.clone(),
        file_name: ctx.file_name.clone(),
        name_span: interface.name_span,
        type_parameters: interface.type_parameters.clone(),
        members: interface.members.clone(),
        resolution_scope: None,
    };

    if ctx
        .type_declarations
        .insert(interface.name.clone(), TypeDeclarationInfo::Interface(info))
        .is_some()
    {
        let mut diagnostic = Diagnostic::ts2300(&interface.name, ctx.file_name.clone());

        if let Some(span) = interface.name_span {
            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
        }

        ctx.push(diagnostic);
    }
}
