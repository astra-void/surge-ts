use oxc_ast::ast::{
    ImportDeclaration, ImportDeclarationSpecifier, ImportOrExportKind, ImportSpecifier,
    ModuleExportName, TSImportEqualsDeclaration, TSModuleReference,
};
use oxc_span::GetSpan;

use crate::{ParsedImportDeclaration, ParsedImportKind, ParsedImportSpecifier};

use super::spans::text_span_from_oxc_span;

pub(crate) fn parse_import_declaration(
    declaration: &ImportDeclaration<'_>,
) -> Option<ParsedImportDeclaration> {
    let module_specifier = declaration.source.value.to_string();
    let module_specifier_span = Some(text_span_from_oxc_span(declaration.source.span));
    let span = Some(text_span_from_oxc_span(declaration.span));

    let Some(specifiers) = declaration.specifiers.as_ref() else {
        return Some(ParsedImportDeclaration {
            kind: ParsedImportKind::SideEffect,
            module_specifier,
            module_specifier_span,
            span,
        });
    };

    let mut parsed_specifiers = Vec::new();
    let mut parsed_default_specifier = None;
    let mut parsed_namespace_specifier = None;

    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                let Some(parsed_specifier) = parse_import_specifier(specifier.as_ref()) else {
                    return Some(ParsedImportDeclaration {
                        kind: ParsedImportKind::Unsupported,
                        module_specifier,
                        module_specifier_span,
                        span,
                    });
                };

                parsed_specifiers.push(parsed_specifier);
            }
            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                if parsed_default_specifier.is_some() || parsed_namespace_specifier.is_some() {
                    return Some(ParsedImportDeclaration {
                        kind: ParsedImportKind::Unsupported,
                        module_specifier,
                        module_specifier_span,
                        span,
                    });
                }

                parsed_default_specifier = Some(ParsedImportKind::Default {
                    local_name: specifier.local.name.to_string(),
                    name_span: Some(text_span_from_oxc_span(specifier.local.span)),
                });
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                if parsed_default_specifier.is_some()
                    || parsed_namespace_specifier.is_some()
                    || !parsed_specifiers.is_empty()
                {
                    return Some(ParsedImportDeclaration {
                        kind: ParsedImportKind::Unsupported,
                        module_specifier,
                        module_specifier_span,
                        span,
                    });
                }

                parsed_namespace_specifier = Some(ParsedImportKind::Namespace {
                    local_name: specifier.local.name.to_string(),
                    name_span: Some(text_span_from_oxc_span(specifier.local.span)),
                    is_type_only: false,
                });
            }
        }
    }

    let is_type_only = matches!(declaration.import_kind, ImportOrExportKind::Type);

    if is_type_only && parsed_default_specifier.is_some() {
        return Some(ParsedImportDeclaration {
            kind: ParsedImportKind::Unsupported,
            module_specifier,
            module_specifier_span,
            span,
        });
    }

    if let Some(default_specifier) = parsed_default_specifier {
        if !parsed_specifiers.is_empty() {
            let ParsedImportKind::Default {
                local_name,
                name_span,
            } = default_specifier
            else {
                unreachable!();
            };

            return Some(ParsedImportDeclaration {
                kind: ParsedImportKind::DefaultAndNamed {
                    local_name,
                    name_span,
                    is_type_only,
                    specifiers: parsed_specifiers,
                },
                module_specifier,
                module_specifier_span,
                span,
            });
        }

        return Some(ParsedImportDeclaration {
            kind: default_specifier,
            module_specifier,
            module_specifier_span,
            span,
        });
    }

    if let Some(namespace_specifier) = parsed_namespace_specifier {
        let kind = match namespace_specifier {
            ParsedImportKind::Namespace {
                local_name,
                name_span,
                ..
            } => ParsedImportKind::Namespace {
                local_name,
                name_span,
                is_type_only,
            },
            _ => namespace_specifier,
        };

        return Some(ParsedImportDeclaration {
            kind,
            module_specifier,
            module_specifier_span,
            span,
        });
    }

    Some(ParsedImportDeclaration {
        kind: ParsedImportKind::Named {
            is_type_only,
            specifiers: parsed_specifiers,
        },
        module_specifier,
        module_specifier_span,
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
        module_specifier_span: Some(text_span_from_oxc_span(declaration.span)),
        span: Some(text_span_from_oxc_span(declaration.span)),
    })
}

fn parse_import_specifier(specifier: &ImportSpecifier<'_>) -> Option<ParsedImportSpecifier> {
    let imported_name = module_export_name_to_string(&specifier.imported);
    let local_name = specifier.local.name.to_string();

    Some(ParsedImportSpecifier {
        imported_name,
        local_name,
        name_span: Some(text_span_from_oxc_span(specifier.imported.span())),
    })
}

fn module_export_name_to_string(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(string_literal) => string_literal.value.to_string(),
    }
}
