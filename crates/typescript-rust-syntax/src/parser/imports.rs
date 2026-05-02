use oxc_ast::ast::{
    ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, ImportSpecifier,
    ModuleExportName, TSImportEqualsDeclaration, TSModuleReference,
};

use crate::{ParsedImportDeclaration, ParsedImportKind, ParsedImportSpecifier};

use super::spans::text_span_from_oxc_span;

pub(crate) fn parse_import_declaration(
    declaration: &ImportDeclaration<'_>,
) -> Option<ParsedImportDeclaration> {
    let module_specifier = declaration.source.value.to_string();
    let span = Some(text_span_from_oxc_span(declaration.span));

    let Some(specifiers) = declaration.specifiers.as_ref() else {
        return Some(ParsedImportDeclaration {
            kind: ParsedImportKind::SideEffect,
            module_specifier,
            span,
        });
    };

    let mut parsed_specifiers = Vec::new();

    for specifier in specifiers {
        let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
            return Some(ParsedImportDeclaration {
                kind: ParsedImportKind::Unsupported,
                module_specifier,
                span,
            });
        };

        let Some(parsed_specifier) = parse_import_specifier(specifier.as_ref()) else {
            return Some(ParsedImportDeclaration {
                kind: ParsedImportKind::Unsupported,
                module_specifier,
                span,
            });
        };

        parsed_specifiers.push(parsed_specifier);
    }

    let is_type_only = matches!(declaration.import_kind, ImportOrExportKind::Type);

    Some(ParsedImportDeclaration {
        kind: ParsedImportKind::Named {
            is_type_only,
            specifiers: parsed_specifiers,
        },
        module_specifier,
        span,
    })
}

pub(crate) fn parse_import_equals_declaration(
    declaration: &TSImportEqualsDeclaration<'_>,
) -> Option<ParsedImportDeclaration> {
    let module_specifier = match &declaration.module_reference {
        TSModuleReference::ExternalModuleReference(reference) => {
            reference.expression.value.to_string()
        }
        _ => String::new(),
    };

    Some(ParsedImportDeclaration {
        kind: ParsedImportKind::Unsupported,
        module_specifier,
        span: Some(text_span_from_oxc_span(declaration.span)),
    })
}

fn parse_import_specifier(specifier: &ImportSpecifier<'_>) -> Option<ParsedImportSpecifier> {
    let imported_name = module_export_name_to_string(&specifier.imported);
    let local_name = specifier.local.name.to_string();

    Some(ParsedImportSpecifier {
        imported_name,
        local_name,
        name_span: Some(text_span_from_oxc_span(specifier.span)),
    })
}

fn module_export_name_to_string(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(string_literal) => string_literal.value.to_string(),
    }
}
