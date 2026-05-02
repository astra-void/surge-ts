use oxc_ast::ast::{
    Declaration, ExportNamedDeclaration, ExportSpecifier, ImportOrExportKind, ModuleExportName,
};
use oxc_span::GetSpan;

use crate::{ParsedExportDeclaration, ParsedExportSpecifier, ParsedStatement};

use super::spans::text_span_from_oxc_span;
use super::{
    parse_function_declaration, parse_interface_declaration, parse_type_alias_declaration,
    parse_variable_declaration,
};

pub(crate) fn parse_export_named_declaration(
    declaration: &ExportNamedDeclaration<'_>,
) -> Option<Vec<ParsedStatement>> {
    let span = Some(text_span_from_oxc_span(declaration.span));

    if declaration.source.is_some() {
        return Some(vec![ParsedStatement::ExportDeclaration(
            ParsedExportDeclaration::Unsupported { span },
        )]);
    }

    if let Some(wrapped_declaration) = declaration.declaration.as_ref() {
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
            module_specifier: None,
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
