use oxc_ast::ast::{
    ArrayPattern, AssignmentOperator, AssignmentTarget, BindingPattern, BindingProperty,
    Declaration, Expression, ExpressionStatement, ModuleDeclaration, ObjectPattern, PropertyKey,
    Statement, TSGlobalDeclaration, TSModuleDeclaration, TSModuleDeclarationBody,
    TSModuleDeclarationName, VariableDeclaration, VariableDeclarationKind,
};

use crate::{
    ParsedAssignment, ParsedDeclareModuleDeclaration, ParsedExportDeclaration, ParsedExpression,
    ParsedNamespaceDeclaration, ParsedStatement, ParsedVariableDeclaration, ParsedVariableKind,
};

mod classes;
mod entry;
mod exports;
mod expressions;
mod function_types;
mod functions;
mod imports;
mod interfaces;
mod reference_directives;
mod spans;
mod types;

use self::classes::parse_class_declaration;
use self::exports::parse_export_named_declaration;
use self::exports::{
    parse_export_all_declaration, parse_export_assignment, parse_export_default_declaration,
};
use self::expressions::{
    parse_call_expression, parse_conditional_expression, parse_expression,
    parse_static_member_expression, parse_unary_expression,
};
use self::functions::parse_function_declaration;
use self::imports::{parse_import_declaration, parse_import_equals_declaration};
use self::interfaces::parse_interface_declaration;
pub use self::reference_directives::{
    extract_reference_path_directives, extract_reference_type_directives,
};
use self::spans::text_span_from_oxc_span;
use self::types::{parse_type_alias_declaration, parse_type_annotation};
pub use entry::parse_source;

fn parse_statement(statement: &Statement<'_>) -> Option<Vec<ParsedStatement>> {
    if let Some(module_declaration) = statement.as_module_declaration() {
        return parse_module_declaration(module_declaration);
    }

    if let Some(declaration) = statement.as_declaration() {
        return parse_declaration(declaration);
    }

    match statement {
        Statement::ExpressionStatement(expression_statement) => {
            parse_expression_statement(expression_statement).map(|statement| vec![statement])
        }
        _ => None,
    }
}

fn parse_module_declaration(
    module_declaration: &ModuleDeclaration<'_>,
) -> Option<Vec<ParsedStatement>> {
    match module_declaration {
        ModuleDeclaration::ImportDeclaration(import) => parse_import_declaration(import)
            .map(|import| vec![ParsedStatement::ImportDeclaration(import)]),
        ModuleDeclaration::ExportNamedDeclaration(export) => parse_export_named_declaration(export),
        ModuleDeclaration::ExportDefaultDeclaration(export) => {
            parse_export_default_declaration(export)
        }
        ModuleDeclaration::ExportAllDeclaration(export) => parse_export_all_declaration(export),
        ModuleDeclaration::TSExportAssignment(export) => parse_export_assignment(export),
        ModuleDeclaration::TSNamespaceExportDeclaration(export) => {
            Some(vec![ParsedStatement::ExportDeclaration(
                ParsedExportDeclaration::Unsupported {
                    span: Some(text_span_from_oxc_span(export.span)),
                },
            )])
        }
    }
}

fn parse_declaration(declaration: &Declaration<'_>) -> Option<Vec<ParsedStatement>> {
    match declaration {
        Declaration::VariableDeclaration(declaration) => {
            Some(parse_variable_declaration(declaration))
        }
        Declaration::FunctionDeclaration(function) => parse_function_declaration(function)
            .map(|function| vec![ParsedStatement::FunctionDeclaration(function)]),
        Declaration::TSTypeAliasDeclaration(type_alias) => parse_type_alias_declaration(type_alias)
            .map(|type_alias| vec![ParsedStatement::TypeAliasDeclaration(type_alias)]),
        Declaration::TSInterfaceDeclaration(interface) => parse_interface_declaration(interface)
            .map(|interface| vec![ParsedStatement::InterfaceDeclaration(interface)]),
        Declaration::ClassDeclaration(class) => parse_class_declaration(class)
            .map(|class| vec![ParsedStatement::ClassDeclaration(class)]),
        Declaration::TSModuleDeclaration(module) => Some(parse_ts_module_declaration(module)),
        Declaration::TSGlobalDeclaration(global) => Some(parse_ts_global_declaration(global)),
        Declaration::TSImportEqualsDeclaration(import_equals) => {
            parse_import_equals_declaration(import_equals)
                .map(|import| vec![ParsedStatement::ImportDeclaration(import)])
        }
        _ => Some(vec![ParsedStatement::UnsupportedDeclaration {
            span: Some(text_span_from_oxc_span(oxc_span::GetSpan::span(
                declaration,
            ))),
        }]),
    }
}

fn parse_variable_declaration(declaration: &VariableDeclaration<'_>) -> Vec<ParsedStatement> {
    let kind = match declaration.kind {
        VariableDeclarationKind::Var => ParsedVariableKind::Var,
        VariableDeclarationKind::Let => ParsedVariableKind::Let,
        VariableDeclarationKind::Const => ParsedVariableKind::Const,
        _ => ParsedVariableKind::Var,
    };

    declaration
        .declarations
        .iter()
        .flat_map(|declarator| {
            let declared_type = declarator
                .type_annotation
                .as_ref()
                .and_then(|annotation| parse_type_annotation(annotation));
            let Some(init) = declarator.init.as_ref() else {
                return parse_binding_pattern_declarations(
                    &declarator.id,
                    None,
                    None,
                    declaration.declare,
                    kind,
                    declared_type,
                );
            };

            let (initializer, initializer_span) = parse_expression(init);
            let initializer_span = Some(text_span_from_oxc_span(initializer_span));

            parse_binding_pattern_declarations(
                &declarator.id,
                Some(initializer),
                initializer_span,
                declaration.declare,
                kind,
                declared_type,
            )
        })
        .collect()
}

fn parse_expression_statement(
    expression_statement: &ExpressionStatement<'_>,
) -> Option<ParsedStatement> {
    match &expression_statement.expression {
        Expression::CallExpression(_) => {
            if let Some(call) = parse_call_expression(match &expression_statement.expression {
                Expression::CallExpression(call_expression) => call_expression,
                _ => unreachable!(),
            }) {
                return Some(ParsedStatement::Call(call));
            }

            let (expression, _) = parse_expression(&expression_statement.expression);
            Some(ParsedStatement::Expression(expression))
        }
        Expression::AssignmentExpression(assignment) => {
            parse_assignment_expression(assignment).map(ParsedStatement::Assignment)
        }
        Expression::UnaryExpression(unary_expression) => {
            parse_unary_expression(unary_expression).map(ParsedStatement::Expression)
        }
        Expression::ConditionalExpression(conditional_expression) => {
            parse_conditional_expression(conditional_expression).map(ParsedStatement::Expression)
        }
        Expression::StaticMemberExpression(member_expression) => {
            parse_static_member_expression(member_expression).map(ParsedStatement::Expression)
        }
        _ => {
            let (expression, _) = parse_expression(&expression_statement.expression);
            Some(ParsedStatement::Expression(expression))
        }
    }
}

fn parse_assignment_expression(
    assignment: &oxc_ast::ast::AssignmentExpression<'_>,
) -> Option<ParsedAssignment> {
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }

    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };

    let (value, value_span) = parse_expression(&assignment.right);

    Some(ParsedAssignment {
        target_name: identifier.name.to_string(),
        target_span: Some(text_span_from_oxc_span(identifier.span)),
        value,
        value_span: Some(text_span_from_oxc_span(value_span)),
    })
}

fn parse_binding_pattern_declarations(
    binding: &BindingPattern<'_>,
    initializer: Option<ParsedExpression>,
    initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
    declared_type: Option<crate::ParsedType>,
) -> Vec<ParsedStatement> {
    match binding {
        BindingPattern::BindingIdentifier(binding_identifier) => {
            vec![ParsedStatement::VariableDeclaration(
                ParsedVariableDeclaration {
                    is_declare,
                    kind,
                    name: binding_identifier.name.to_string(),
                    name_span: Some(text_span_from_oxc_span(binding_identifier.span)),
                    declared_type,
                    initializer,
                    initializer_span,
                },
            )]
        }
        BindingPattern::AssignmentPattern(assignment_pattern) => {
            parse_binding_pattern_declarations(
                &assignment_pattern.left,
                initializer,
                initializer_span,
                is_declare,
                kind,
                declared_type,
            )
        }
        BindingPattern::ObjectPattern(object_pattern) => parse_object_pattern_declarations(
            object_pattern,
            initializer,
            initializer_span,
            is_declare,
            kind,
        ),
        BindingPattern::ArrayPattern(array_pattern) => parse_array_pattern_declarations(
            array_pattern,
            initializer,
            initializer_span,
            is_declare,
            kind,
        ),
    }
}

fn parse_object_pattern_declarations(
    object_pattern: &ObjectPattern<'_>,
    initializer: Option<ParsedExpression>,
    initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
) -> Vec<ParsedStatement> {
    let Some(initializer) = initializer else {
        return Vec::new();
    };

    let mut declarations = Vec::new();

    for property in &object_pattern.properties {
        declarations.extend(parse_object_binding_property_declarations(
            property,
            initializer.clone(),
            initializer_span,
            is_declare,
            kind,
        ));
    }

    // `const { a, ...rest } = obj` binds `rest` to the remaining properties.
    if let Some(rest) = object_pattern.rest.as_deref() {
        declarations.extend(parse_binding_pattern_declarations(
            &rest.argument,
            Some(initializer),
            initializer_span,
            is_declare,
            kind,
            None,
        ));
    }

    declarations
}

fn parse_object_binding_property_declarations(
    property: &BindingProperty<'_>,
    source_initializer: ParsedExpression,
    source_initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
) -> Vec<ParsedStatement> {
    let PropertyKey::StaticIdentifier(identifier) = &property.key else {
        return Vec::new();
    };

    let property_initializer = ParsedExpression::PropertyAccess {
        object: Box::new(source_initializer),
        object_span: source_initializer_span,
        property_name: identifier.name.to_string(),
        property_span: Some(text_span_from_oxc_span(identifier.span)),
    };

    parse_binding_pattern_declarations(
        &property.value,
        Some(property_initializer),
        Some(text_span_from_oxc_span(identifier.span)),
        is_declare,
        kind,
        None,
    )
}

fn parse_array_pattern_declarations(
    array_pattern: &ArrayPattern<'_>,
    initializer: Option<ParsedExpression>,
    initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
) -> Vec<ParsedStatement> {
    let Some(initializer) = initializer else {
        return Vec::new();
    };

    let mut declarations = Vec::new();

    for (index, element) in array_pattern.elements.iter().enumerate() {
        let Some(element) = element else {
            continue;
        };

        let element_initializer = match &initializer {
            ParsedExpression::Identifier { name, .. } => ParsedExpression::IndexAccess {
                object_name: name.clone(),
                object_span: initializer_span,
                index: Box::new(ParsedExpression::NumberLiteral(index.to_string())),
                index_span: initializer_span,
            },
            _ => initializer.clone(),
        };

        declarations.extend(parse_binding_pattern_declarations(
            element,
            Some(element_initializer),
            initializer_span,
            is_declare,
            kind,
            None,
        ));
    }

    declarations
}

fn parse_ts_module_declaration(module: &TSModuleDeclaration<'_>) -> Vec<ParsedStatement> {
    use oxc_span::GetSpan;
    let module_specifier = match &module.id {
        TSModuleDeclarationName::StringLiteral(literal) => literal.value.to_string(),
        // `namespace JSX { ... }` / `module Foo { ... }` (identifier-named). The body
        // is preserved so the checker can resolve qualified members such as
        // `JSX.IntrinsicElements`.
        TSModuleDeclarationName::Identifier(identifier) => {
            return parse_ts_namespace_declaration(
                identifier.name.to_string(),
                Some(text_span_from_oxc_span(identifier.span)),
                module,
            );
        }
    };

    if module_specifier.contains('*') {
        return vec![ParsedStatement::UnsupportedDeclaration {
            span: Some(text_span_from_oxc_span(module.span)),
        }];
    }

    let statements = match &module.body {
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => block
            .body
            .iter()
            .filter_map(parse_statement)
            .flatten()
            .collect(),
        _ => {
            return vec![ParsedStatement::UnsupportedDeclaration {
                span: Some(text_span_from_oxc_span(module.span)),
            }];
        }
    };

    vec![ParsedStatement::DeclareModuleDeclaration(
        ParsedDeclareModuleDeclaration {
            module_specifier,
            module_specifier_span: Some(text_span_from_oxc_span(module.id.span())),
            statements,
            span: Some(text_span_from_oxc_span(module.span)),
        },
    )]
}

fn parse_ts_namespace_declaration(
    name: String,
    name_span: Option<crate::TextSpan>,
    module: &TSModuleDeclaration<'_>,
) -> Vec<ParsedStatement> {
    let statements = match &module.body {
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => block
            .body
            .iter()
            .filter_map(parse_statement)
            .flatten()
            .collect(),
        // `namespace A.B { ... }` nests as a module body; flatten it into a
        // dotted-name namespace so members resolve as `A.B.Member`.
        Some(TSModuleDeclarationBody::TSModuleDeclaration(inner)) => {
            let inner_name = match &inner.id {
                TSModuleDeclarationName::Identifier(identifier) => identifier.name.to_string(),
                TSModuleDeclarationName::StringLiteral(literal) => literal.value.to_string(),
            };
            parse_ts_namespace_declaration(format!("{name}.{inner_name}"), name_span, inner)
        }
        None => Vec::new(),
    };

    vec![ParsedStatement::NamespaceDeclaration(
        ParsedNamespaceDeclaration {
            name,
            name_span,
            statements,
            span: Some(text_span_from_oxc_span(module.span)),
        },
    )]
}

fn parse_ts_global_declaration(global: &TSGlobalDeclaration<'_>) -> Vec<ParsedStatement> {
    let statements = global
        .body
        .body
        .iter()
        .filter_map(parse_statement)
        .flatten()
        .collect();

    vec![ParsedStatement::DeclareModuleDeclaration(
        ParsedDeclareModuleDeclaration {
            module_specifier: "global".to_string(),
            module_specifier_span: Some(text_span_from_oxc_span(global.global_span)),
            statements,
            span: Some(text_span_from_oxc_span(global.span)),
        },
    )]
}
