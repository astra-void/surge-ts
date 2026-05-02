use oxc_ast::ast::{
    AssignmentOperator, AssignmentTarget, Declaration, Expression, ExpressionStatement, Statement,
    VariableDeclaration, VariableDeclarationKind,
};

use crate::{ParsedAssignment, ParsedStatement, ParsedVariableDeclaration, ParsedVariableKind};

mod entry;
mod expressions;
mod function_types;
mod functions;
mod interfaces;
mod spans;
mod types;

use self::expressions::{
    parse_call_expression, parse_conditional_expression, parse_expression,
    parse_static_member_expression, parse_unary_expression,
};
use self::functions::parse_function_declaration;
use self::interfaces::parse_interface_declaration;
use self::spans::text_span_from_oxc_span;
use self::types::{parse_type_alias_declaration, parse_type_annotation};
pub use entry::parse_source;

fn parse_statement(statement: &Statement<'_>) -> Option<Vec<ParsedStatement>> {
    match statement {
        Statement::ExpressionStatement(expression_statement) => {
            parse_expression_statement(expression_statement).map(|statement| vec![statement])
        }
        _ => {
            let declaration = statement.as_declaration()?;

            match declaration {
                Declaration::VariableDeclaration(declaration) => {
                    Some(parse_variable_declaration(declaration))
                }
                Declaration::FunctionDeclaration(function) => parse_function_declaration(function)
                    .map(|function| vec![ParsedStatement::FunctionDeclaration(function)]),
                Declaration::TSTypeAliasDeclaration(type_alias) => {
                    parse_type_alias_declaration(type_alias)
                        .map(|type_alias| vec![ParsedStatement::TypeAliasDeclaration(type_alias)])
                }
                Declaration::TSInterfaceDeclaration(interface) => {
                    parse_interface_declaration(interface)
                        .map(|interface| vec![ParsedStatement::InterfaceDeclaration(interface)])
                }
                _ => None,
            }
        }
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
        .filter_map(|declarator| {
            let Some(binding_identifier) = declarator.id.get_binding_identifier() else {
                return None;
            };

            let name = binding_identifier.name.to_string();
            let name_span = Some(text_span_from_oxc_span(binding_identifier.span));
            let declared_type = declarator
                .type_annotation
                .as_ref()
                .and_then(|annotation| parse_type_annotation(annotation));
            let (initializer, initializer_span) = declarator
                .init
                .as_ref()
                .map(parse_expression)
                .map(|(initializer, span)| (Some(initializer), Some(text_span_from_oxc_span(span))))
                .unwrap_or((None, None));

            Some(ParsedStatement::VariableDeclaration(
                ParsedVariableDeclaration {
                    kind,
                    name,
                    name_span,
                    declared_type,
                    initializer,
                    initializer_span,
                },
            ))
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
        _ => None,
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
