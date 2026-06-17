use oxc_ast::ast::{
    Declaration, ExportAllDeclaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
    ExportNamedDeclaration, ExportSpecifier, Expression, ImportOrExportKind, ModuleExportName,
    TSExportAssignment,
};
use oxc_span::GetSpan;

use crate::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedExportSpecifier,
    ParsedExpression, ParsedStatement,
};

use super::expressions::{
    parse_array_expression, parse_conditional_expression, parse_expression,
    parse_object_properties, parse_static_member_expression, parse_unary_expression,
};
use super::spans::text_span_from_oxc_span;
use super::{
    parse_function_declaration, parse_interface_declaration, parse_type_alias_declaration,
    parse_variable_declaration,
};

pub(crate) fn parse_export_named_declaration(
    declaration: &ExportNamedDeclaration<'_>,
) -> Option<Vec<ParsedStatement>> {
    let span = Some(text_span_from_oxc_span(declaration.span));

    if let Some(wrapped_declaration) = declaration.declaration.as_ref() {
        if declaration.source.is_some() {
            return Some(vec![ParsedStatement::ExportDeclaration(
                ParsedExportDeclaration::Unsupported { span },
            )]);
        }

        return parse_exported_declaration(wrapped_declaration, declaration.export_kind).or_else(
            || {
                Some(vec![ParsedStatement::ExportDeclaration(
                    ParsedExportDeclaration::Unsupported { span },
                )])
            },
        );
    }

    if declaration.specifiers.is_empty() {
        return Some(vec![ParsedStatement::ExportDeclaration(
            ParsedExportDeclaration::Empty { span },
        )]);
    }

    let mut specifiers = Vec::new();

    for specifier in &declaration.specifiers {
        let Some(parsed_specifier) = parse_export_specifier(specifier) else {
            return Some(vec![ParsedStatement::ExportDeclaration(
                ParsedExportDeclaration::Unsupported { span },
            )]);
        };

        specifiers.push(parsed_specifier);
    }

    Some(vec![ParsedStatement::ExportDeclaration(
        ParsedExportDeclaration::Named {
            is_type_only: matches!(declaration.export_kind, ImportOrExportKind::Type),
            specifiers,
            module_specifier: declaration
                .source
                .as_ref()
                .map(|source| source.value.to_string()),
            module_specifier_span: declaration
                .source
                .as_ref()
                .map(|source| text_span_from_oxc_span(source.span)),
            span,
        },
    )])
}

pub(crate) fn parse_export_default_declaration(
    declaration: &ExportDefaultDeclaration<'_>,
) -> Option<Vec<ParsedStatement>> {
    let span = Some(text_span_from_oxc_span(declaration.span));

    let parsed_declaration = match &declaration.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            let Some(function) = parse_function_declaration(function) else {
                return Some(vec![ParsedStatement::ExportDeclaration(
                    ParsedExportDeclaration::Default {
                        declaration: ParsedDefaultExportDeclaration::Unsupported {
                            span: Some(text_span_from_oxc_span(declaration.span)),
                        },
                        span,
                    },
                )]);
            };

            ParsedDefaultExportDeclaration::Function(function)
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            match super::classes::parse_class_declaration(class) {
                Some(class) => ParsedDefaultExportDeclaration::Class(class),
                None => ParsedDefaultExportDeclaration::Unsupported {
                    span: Some(text_span_from_oxc_span(declaration.span)),
                },
            }
        }
        ExportDefaultDeclarationKind::BooleanLiteral(boolean_literal) => {
            ParsedDefaultExportDeclaration::Expression(ParsedExpression::BooleanLiteral(
                boolean_literal.value,
            ))
        }
        ExportDefaultDeclarationKind::NumericLiteral(numeric_literal) => {
            ParsedDefaultExportDeclaration::Expression(ParsedExpression::NumberLiteral(
                numeric_literal.value.to_string(),
            ))
        }
        ExportDefaultDeclarationKind::StringLiteral(string_literal) => {
            ParsedDefaultExportDeclaration::Expression(ParsedExpression::StringLiteral(
                string_literal.value.to_string(),
            ))
        }
        ExportDefaultDeclarationKind::Identifier(identifier) => {
            ParsedDefaultExportDeclaration::Expression(ParsedExpression::Identifier {
                name: identifier.name.to_string(),
                span: Some(text_span_from_oxc_span(identifier.span)),
            })
        }
        ExportDefaultDeclarationKind::ObjectExpression(object_expression) => {
            ParsedDefaultExportDeclaration::Expression(ParsedExpression::ObjectLiteral {
                properties: parse_object_properties(object_expression),
                span: Some(text_span_from_oxc_span(object_expression.span())),
            })
        }
        ExportDefaultDeclarationKind::ArrayExpression(array_expression) => {
            let Some(parsed_expression) = parse_array_expression(array_expression) else {
                return Some(vec![ParsedStatement::ExportDeclaration(
                    ParsedExportDeclaration::Default {
                        declaration: ParsedDefaultExportDeclaration::Unsupported { span },
                        span,
                    },
                )]);
            };

            ParsedDefaultExportDeclaration::Expression(parsed_expression)
        }
        ExportDefaultDeclarationKind::UnaryExpression(unary_expression) => {
            let Some(parsed_expression) = parse_unary_expression(unary_expression) else {
                return Some(vec![ParsedStatement::ExportDeclaration(
                    ParsedExportDeclaration::Default {
                        declaration: ParsedDefaultExportDeclaration::Unsupported { span },
                        span,
                    },
                )]);
            };

            ParsedDefaultExportDeclaration::Expression(parsed_expression)
        }
        ExportDefaultDeclarationKind::ConditionalExpression(conditional_expression) => {
            let Some(parsed_expression) = parse_conditional_expression(conditional_expression)
            else {
                return Some(vec![ParsedStatement::ExportDeclaration(
                    ParsedExportDeclaration::Default {
                        declaration: ParsedDefaultExportDeclaration::Unsupported { span },
                        span,
                    },
                )]);
            };

            ParsedDefaultExportDeclaration::Expression(parsed_expression)
        }
        ExportDefaultDeclarationKind::StaticMemberExpression(member_expression) => {
            let Some(parsed_expression) = parse_static_member_expression(member_expression) else {
                return Some(vec![ParsedStatement::ExportDeclaration(
                    ParsedExportDeclaration::Default {
                        declaration: ParsedDefaultExportDeclaration::Unsupported { span },
                        span,
                    },
                )]);
            };

            ParsedDefaultExportDeclaration::Expression(parsed_expression)
        }
        ExportDefaultDeclarationKind::ParenthesizedExpression(parenthesized_expression) => {
            let (parsed_expression, _) = parse_expression(&parenthesized_expression.expression);
            ParsedDefaultExportDeclaration::Expression(parsed_expression)
        }
        _ => {
            return Some(vec![ParsedStatement::ExportDeclaration(
                ParsedExportDeclaration::Default {
                    declaration: ParsedDefaultExportDeclaration::Unsupported { span },
                    span,
                },
            )]);
        }
    };

    Some(vec![ParsedStatement::ExportDeclaration(
        ParsedExportDeclaration::Default {
            declaration: parsed_declaration,
            span,
        },
    )])
}

pub(crate) fn parse_export_assignment(
    declaration: &TSExportAssignment<'_>,
) -> Option<Vec<ParsedStatement>> {
    let span = Some(text_span_from_oxc_span(declaration.span));

    // Declaration-lite `export = identifier`. Any non-identifier target
    // (`export = require(...)`, object literals, member access, etc.) stays
    // unsupported.
    let Expression::Identifier(identifier) = &declaration.expression else {
        return Some(vec![ParsedStatement::ExportDeclaration(
            ParsedExportDeclaration::Unsupported { span },
        )]);
    };

    Some(vec![ParsedStatement::ExportDeclaration(
        ParsedExportDeclaration::Equals {
            exported_name: identifier.name.to_string(),
            exported_name_span: Some(text_span_from_oxc_span(identifier.span)),
            span,
        },
    )])
}

pub(crate) fn parse_export_all_declaration(
    declaration: &ExportAllDeclaration<'_>,
) -> Option<Vec<ParsedStatement>> {
    let span = Some(text_span_from_oxc_span(declaration.span));
    let module_specifier = declaration.source.value.to_string();
    let module_specifier_span = Some(text_span_from_oxc_span(declaration.source.span));

    if matches!(declaration.export_kind, ImportOrExportKind::Type) {
        return Some(vec![ParsedStatement::ExportDeclaration(
            ParsedExportDeclaration::Unsupported { span },
        )]);
    }

    if let Some(exported) = declaration.exported.as_ref() {
        return Some(vec![ParsedStatement::ExportDeclaration(
            ParsedExportDeclaration::Namespace {
                exported_name: module_export_name_to_string(exported),
                exported_name_span: Some(text_span_from_oxc_span(exported.span())),
                module_specifier,
                module_specifier_span,
                span,
            },
        )]);
    }

    Some(vec![ParsedStatement::ExportDeclaration(
        ParsedExportDeclaration::All {
            module_specifier,
            module_specifier_span,
            span,
        },
    )])
}

fn parse_exported_declaration(
    declaration: &Declaration<'_>,
    export_kind: ImportOrExportKind,
) -> Option<Vec<ParsedStatement>> {
    let is_type_only = matches!(export_kind, ImportOrExportKind::Type);

    let statements = match declaration {
        Declaration::VariableDeclaration(variable) => parse_variable_declaration(variable),
        Declaration::FunctionDeclaration(function) => parse_function_declaration(function)
            .map(|function| vec![ParsedStatement::FunctionDeclaration(function)])?,
        Declaration::TSTypeAliasDeclaration(type_alias) => parse_type_alias_declaration(type_alias)
            .map(|type_alias| vec![ParsedStatement::TypeAliasDeclaration(type_alias)])?,
        Declaration::TSInterfaceDeclaration(interface) => parse_interface_declaration(interface)
            .map(|interface| vec![ParsedStatement::InterfaceDeclaration(interface)])?,
        Declaration::ClassDeclaration(class) => super::classes::parse_class_declaration(class)
            .map(|class| vec![ParsedStatement::ClassDeclaration(class)])?,
        _ => return None,
    };

    Some(wrap_exported_statements(statements, is_type_only))
}

fn parse_export_specifier(specifier: &ExportSpecifier<'_>) -> Option<ParsedExportSpecifier> {
    let local_name = module_export_name_to_string(&specifier.local);
    let exported_name = module_export_name_to_string(&specifier.exported);

    Some(ParsedExportSpecifier {
        local_name,
        exported_name,
        name_span: Some(text_span_from_oxc_span(specifier.local.span())),
        is_type_only: matches!(specifier.export_kind, ImportOrExportKind::Type),
    })
}

fn module_export_name_to_string(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(string_literal) => string_literal.value.to_string(),
    }
}

fn wrap_exported_statements(
    statements: Vec<ParsedStatement>,
    is_type_only: bool,
) -> Vec<ParsedStatement> {
    statements
        .into_iter()
        .map(|statement| {
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
                declaration: Box::new(statement),
                is_type_only,
            })
        })
        .collect()
}
