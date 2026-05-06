use oxc_ast::ast::{
    BindingPattern, BindingProperty, BlockStatement, Declaration, Expression, ExpressionStatement,
    FormalParameter, Function, IfStatement, ObjectPattern, PropertyKey, Statement,
    VariableDeclaration, WhileStatement,
};

use crate::{
    ParsedBindingName, ParsedExpression, ParsedFunctionBodyStatement, ParsedFunctionDeclaration,
    ParsedFunctionParameter, ParsedIfStatement, ParsedObjectBindingElement,
    ParsedObjectBindingPattern, ParsedReturnStatement, ParsedWhileStatement,
};

use super::expressions::parse_expression;
use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_parameters};

pub(crate) fn parse_function_declaration(
    function: &Function<'_>,
) -> Option<ParsedFunctionDeclaration> {
    let id = function.id.as_ref()?;
    let parameters = function
        .params
        .items
        .iter()
        .filter_map(parse_function_parameter)
        .collect();
    let return_type = function
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation));
    let body = function
        .body
        .as_ref()
        .map(|body| parse_statement_list_as_function_body(&body.statements))
        .unwrap_or_default();

    Some(ParsedFunctionDeclaration {
        is_declare: function.declare,
        name: id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(id.span)),
        type_parameters: parse_type_parameters(function.type_parameters.as_deref()),
        parameters,
        return_type,
        body,
    })
}

fn parse_return_statement(statement: &Statement<'_>) -> Option<ParsedReturnStatement> {
    let Statement::ReturnStatement(return_statement) = statement else {
        return None;
    };

    let (expression, expression_span) = return_statement
        .argument
        .as_ref()
        .map(parse_expression)
        .map(|(expression, span)| (Some(expression), Some(text_span_from_oxc_span(span))))
        .unwrap_or((None, None));

    Some(ParsedReturnStatement {
        expression,
        expression_span,
    })
}

fn parse_function_body_statement(
    statement: &Statement<'_>,
) -> Option<Vec<ParsedFunctionBodyStatement>> {
    match statement {
        Statement::BlockStatement(block) => Some(vec![ParsedFunctionBodyStatement::Block(
            parse_block_statement_as_function_body(block),
        )]),
        Statement::IfStatement(if_statement) => parse_if_statement(if_statement)
            .map(|if_statement| vec![ParsedFunctionBodyStatement::If(if_statement)]),
        Statement::WhileStatement(while_statement) => parse_while_statement(while_statement)
            .map(|while_statement| vec![ParsedFunctionBodyStatement::While(while_statement)]),
        Statement::ReturnStatement(_) => parse_return_statement(statement)
            .map(|return_statement| vec![ParsedFunctionBodyStatement::Return(return_statement)]),
        Statement::ExpressionStatement(expression_statement) => {
            parse_expression_statement_as_function_body(expression_statement)
        }
        _ => {
            let declaration = statement.as_declaration()?;

            match declaration {
                Declaration::VariableDeclaration(declaration) => {
                    Some(parse_variable_declaration_as_function_body(declaration))
                }
                _ => None,
            }
        }
    }
}

pub(crate) fn parse_statement_list_as_function_body(
    statements: &[Statement<'_>],
) -> Vec<ParsedFunctionBodyStatement> {
    statements
        .iter()
        .filter_map(parse_function_body_statement)
        .flatten()
        .collect()
}

fn parse_single_statement_as_function_body(
    statement: &Statement<'_>,
) -> Vec<ParsedFunctionBodyStatement> {
    parse_function_body_statement(statement).unwrap_or_default()
}

fn parse_block_statement_as_function_body(
    block: &BlockStatement<'_>,
) -> Vec<ParsedFunctionBodyStatement> {
    parse_statement_list_as_function_body(&block.body)
}

fn parse_expression_statement_as_function_body(
    expression_statement: &ExpressionStatement<'_>,
) -> Option<Vec<ParsedFunctionBodyStatement>> {
    match &expression_statement.expression {
        Expression::AssignmentExpression(assignment) => {
            super::parse_assignment_expression(assignment)
                .map(|assignment| vec![ParsedFunctionBodyStatement::Assignment(assignment)])
        }
        _ => {
            let (expression, _) = parse_expression(&expression_statement.expression);

            if expression == ParsedExpression::Unknown {
                return None;
            }

            Some(vec![ParsedFunctionBodyStatement::Expression(expression)])
        }
    }
}

fn parse_variable_declaration_as_function_body(
    declaration: &VariableDeclaration<'_>,
) -> Vec<ParsedFunctionBodyStatement> {
    super::parse_variable_declaration(declaration)
        .into_iter()
        .filter_map(|statement| match statement {
            crate::ParsedStatement::VariableDeclaration(variable) => {
                Some(ParsedFunctionBodyStatement::VariableDeclaration(variable))
            }
            _ => None,
        })
        .collect()
}

fn parse_if_statement(if_statement: &IfStatement<'_>) -> Option<ParsedIfStatement> {
    let (condition, condition_span) = parse_expression(&if_statement.test);
    let then_body = parse_branch_body(&if_statement.consequent);
    let else_body = if_statement
        .alternate
        .as_ref()
        .map(parse_branch_body)
        .unwrap_or_default();

    Some(ParsedIfStatement {
        condition,
        condition_span: Some(text_span_from_oxc_span(condition_span)),
        then_body,
        else_body,
    })
}

fn parse_while_statement(while_statement: &WhileStatement<'_>) -> Option<ParsedWhileStatement> {
    let (condition, condition_span) = parse_expression(&while_statement.test);
    let body = parse_branch_body(&while_statement.body);

    Some(ParsedWhileStatement {
        condition,
        condition_span: Some(text_span_from_oxc_span(condition_span)),
        body,
    })
}

fn parse_branch_body(statement: &Statement<'_>) -> Vec<ParsedFunctionBodyStatement> {
    match statement {
        Statement::BlockStatement(block) => parse_block_statement_as_function_body(block),
        _ => parse_single_statement_as_function_body(statement),
    }
}

pub(crate) fn parse_function_parameter(
    parameter: &FormalParameter<'_>,
) -> Option<ParsedFunctionParameter> {
    let declared_type = parameter
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation));

    Some(ParsedFunctionParameter {
        binding_name: parse_binding_name(&parameter.pattern),
        declared_type,
    })
}

pub(crate) fn parse_binding_name(binding: &BindingPattern<'_>) -> ParsedBindingName {
    match binding {
        BindingPattern::BindingIdentifier(binding) => ParsedBindingName::Identifier {
            name: binding.name.to_string(),
            span: Some(text_span_from_oxc_span(binding.span)),
        },
        BindingPattern::ObjectPattern(object_pattern) => {
            ParsedBindingName::ObjectPattern(parse_object_binding_pattern(object_pattern))
        }
        BindingPattern::AssignmentPattern(assignment_pattern) => {
            parse_binding_name(&assignment_pattern.left)
        }
        BindingPattern::ArrayPattern(array_pattern) => ParsedBindingName::Unsupported {
            span: Some(text_span_from_oxc_span(array_pattern.span)),
        },
    }
}

fn parse_object_binding_pattern(object_pattern: &ObjectPattern<'_>) -> ParsedObjectBindingPattern {
    ParsedObjectBindingPattern {
        elements: object_pattern
            .properties
            .iter()
            .filter_map(parse_object_binding_element)
            .collect(),
        span: Some(text_span_from_oxc_span(object_pattern.span)),
    }
}

fn parse_object_binding_element(
    property: &BindingProperty<'_>,
) -> Option<ParsedObjectBindingElement> {
    let property_name = match &property.key {
        PropertyKey::StaticIdentifier(identifier) => identifier.name.to_string(),
        _ => {
            return Some(ParsedObjectBindingElement {
                property_name: "<unsupported>".to_string(),
                binding_name: ParsedBindingName::Unsupported {
                    span: Some(text_span_from_oxc_span(property.span)),
                },
                name_span: Some(text_span_from_oxc_span(property.span)),
                has_default: false,
                span: Some(text_span_from_oxc_span(property.span)),
            });
        }
    };

    let has_default = matches!(&property.value, BindingPattern::AssignmentPattern(_));
    let binding_name = match &property.value {
        BindingPattern::AssignmentPattern(assignment) => parse_binding_name(&assignment.left),
        _ => parse_binding_name(&property.value),
    };
    let name_span = match &binding_name {
        ParsedBindingName::Identifier { span, .. } => *span,
        ParsedBindingName::ObjectPattern(pattern) => pattern.span,
        ParsedBindingName::Unsupported { span } => *span,
    };

    Some(ParsedObjectBindingElement {
        property_name,
        binding_name,
        name_span,
        has_default,
        span: Some(text_span_from_oxc_span(property.span)),
    })
}
