use oxc_ast::ast::{
    AssignmentOperator, AssignmentTarget, BindingPattern, BindingProperty, BlockStatement,
    CatchClause, Declaration, Expression, ExpressionStatement, ForOfStatement, ForStatementLeft,
    FormalParameter, Function, IfStatement, ObjectPattern, PropertyKey, Statement, SwitchCase,
    SwitchStatement, ThrowStatement, TryStatement, VariableDeclaration, WhileStatement,
};
use oxc_span::GetSpan;

use crate::{
    ParsedArrayBindingPattern, ParsedBindingName, ParsedExpression, ParsedForOfStatement,
    ParsedFunctionBodyStatement, ParsedFunctionDeclaration, ParsedFunctionParameter,
    ParsedIfStatement, ParsedObjectBindingElement, ParsedObjectBindingPattern,
    ParsedReturnStatement, ParsedSwitchCase, ParsedSwitchStatement, ParsedThisPropertyAssignment,
    ParsedThrowStatement, ParsedTryStatement, ParsedWhileStatement,
};

use super::expressions::parse_expression;
use super::spans::text_span_from_oxc_span;
use super::types::{parse_type_annotation, parse_type_parameters};

pub(crate) fn parse_function_declaration(
    function: &Function<'_>,
) -> Option<ParsedFunctionDeclaration> {
    let id = function.id.as_ref()?;
    parse_function_declaration_named(
        function,
        id.name.to_string(),
        Some(text_span_from_oxc_span(id.span)),
    )
}

/// Parse a function whose binding name is supplied by the caller — used for an
/// anonymous `export default function () {}`, which tsc binds as `default`.
pub(crate) fn parse_function_declaration_named(
    function: &Function<'_>,
    name: String,
    name_span: Option<crate::TextSpan>,
) -> Option<ParsedFunctionDeclaration> {
    let mut parameters: Vec<_> = function
        .params
        .items
        .iter()
        .filter_map(parse_function_parameter)
        .collect();
    if let Some(rest) = function.params.rest.as_deref() {
        if let Some(rest_parameter) = parse_rest_function_parameter(rest) {
            parameters.push(rest_parameter);
        }
    }
    let return_type = function
        .return_type
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation));
    let return_type_span = function
        .return_type
        .as_ref()
        .map(|annotation| text_span_from_oxc_span(annotation.type_annotation.span()));
    let body = function
        .body
        .as_ref()
        .map(|body| parse_statement_list_as_function_body(&body.statements))
        .unwrap_or_default();

    Some(ParsedFunctionDeclaration {
        is_declare: function.declare,
        name,
        name_span,
        type_parameters: parse_type_parameters(function.type_parameters.as_deref()),
        parameters,
        return_type,
        return_type_span,
        body,
        has_body: function.body.is_some(),
        body_reads: function
            .body
            .as_ref()
            .map(|body| super::reads::collect_function_body_reads(body))
            .unwrap_or_default(),
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
            .map(|if_statement| vec![ParsedFunctionBodyStatement::If(Box::new(if_statement))]),
        Statement::WhileStatement(while_statement) => {
            parse_while_statement(while_statement).map(|while_statement| {
                vec![ParsedFunctionBodyStatement::While(Box::new(
                    while_statement,
                ))]
            })
        }
        Statement::ForOfStatement(for_of_statement) => parse_for_of_statement(for_of_statement)
            .map(|for_of_statement| {
                vec![ParsedFunctionBodyStatement::ForOf(Box::new(
                    for_of_statement,
                ))]
            }),
        Statement::SwitchStatement(switch_statement) => parse_switch_statement(switch_statement)
            .map(|switch_statement| {
                vec![ParsedFunctionBodyStatement::Switch(Box::new(
                    switch_statement,
                ))]
            }),
        Statement::ThrowStatement(throw_statement) => {
            parse_throw_statement(throw_statement).map(|throw_statement| {
                vec![ParsedFunctionBodyStatement::Throw(Box::new(
                    throw_statement,
                ))]
            })
        }
        Statement::TryStatement(try_statement) => parse_try_statement(try_statement)
            .map(|try_statement| vec![ParsedFunctionBodyStatement::Try(Box::new(try_statement))]),
        Statement::ReturnStatement(_) => {
            parse_return_statement(statement).map(|return_statement| {
                vec![ParsedFunctionBodyStatement::Return(Box::new(
                    return_statement,
                ))]
            })
        }
        Statement::ContinueStatement(_) => Some(vec![ParsedFunctionBodyStatement::Continue]),
        Statement::BreakStatement(_) => Some(vec![ParsedFunctionBodyStatement::Break]),
        Statement::ExpressionStatement(expression_statement) => {
            parse_expression_statement_as_function_body(expression_statement)
        }
        _ => {
            let declaration = statement.as_declaration()?;

            match declaration {
                Declaration::VariableDeclaration(declaration) => {
                    Some(parse_variable_declaration_as_function_body(declaration))
                }
                Declaration::FunctionDeclaration(function) => parse_function_declaration(function)
                    .map(|function| {
                        vec![ParsedFunctionBodyStatement::Function(Box::new(function))]
                    }),
                Declaration::TSTypeAliasDeclaration(alias) => {
                    super::types::parse_type_alias_declaration(alias)
                        .map(|alias| vec![ParsedFunctionBodyStatement::TypeAlias(Box::new(alias))])
                }
                Declaration::TSInterfaceDeclaration(interface) => {
                    super::interfaces::parse_interface_declaration(interface).map(|interface| {
                        vec![ParsedFunctionBodyStatement::Interface(Box::new(interface))]
                    })
                }
                Declaration::ClassDeclaration(class) => {
                    super::classes::parse_class_declaration(class)
                        .map(|class| vec![ParsedFunctionBodyStatement::Class(Box::new(class))])
                }
                Declaration::TSEnumDeclaration(enum_declaration) => Some(
                    super::enums::parse_enum_declaration_as_function_body(enum_declaration),
                ),
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
            if let Some(this_assignment) = parse_this_property_assignment(assignment) {
                return Some(vec![ParsedFunctionBodyStatement::ThisPropertyAssignment(
                    Box::new(this_assignment),
                )]);
            }

            if let Some(member_assignment) = parse_member_assignment(assignment) {
                return Some(vec![ParsedFunctionBodyStatement::MemberAssignment(
                    Box::new(member_assignment),
                )]);
            }

            super::parse_assignment_expression(assignment).map(|assignment| {
                vec![ParsedFunctionBodyStatement::Assignment(Box::new(
                    assignment,
                ))]
            })
        }
        _ => {
            let (expression, _) = parse_expression(&expression_statement.expression);

            if expression == ParsedExpression::Unknown {
                return None;
            }

            Some(vec![ParsedFunctionBodyStatement::Expression(Box::new(
                expression,
            ))])
        }
    }
}

/// `o.p = v` where the target is a member of something other than `this`.
/// Without this the whole statement was dropped, so neither the assignment's own
/// type check nor the narrowing it establishes for the code after it happened.
fn parse_member_assignment(
    assignment: &oxc_ast::ast::AssignmentExpression<'_>,
) -> Option<crate::ParsedMemberAssignment> {
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }

    let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
        return None;
    };

    let (object, object_span) = parse_expression(&member.object);
    if object == ParsedExpression::Unknown {
        return None;
    }
    let target = ParsedExpression::PropertyAccess {
        object: Box::new(object),
        object_span: Some(text_span_from_oxc_span(object_span)),
        property_name: member.property.name.to_string(),
        property_span: Some(text_span_from_oxc_span(member.property.span)),
        is_bracketed: false,
    };

    let (value, value_span) = parse_expression(&assignment.right);
    if value == ParsedExpression::Unknown {
        return None;
    }

    Some(crate::ParsedMemberAssignment {
        target,
        target_span: Some(text_span_from_oxc_span(member.span)),
        value,
        value_span: Some(text_span_from_oxc_span(value_span)),
    })
}

fn parse_this_property_assignment(
    assignment: &oxc_ast::ast::AssignmentExpression<'_>,
) -> Option<ParsedThisPropertyAssignment> {
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }

    let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
        return None;
    };

    if !matches!(&member.object, Expression::ThisExpression(_)) {
        return None;
    }

    let (value, value_span) = parse_expression(&assignment.right);

    Some(ParsedThisPropertyAssignment {
        property_name: member.property.name.to_string(),
        property_span: Some(text_span_from_oxc_span(member.property.span)),
        value,
        value_span: Some(text_span_from_oxc_span(value_span)),
    })
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

fn parse_for_of_statement(for_of_statement: &ForOfStatement<'_>) -> Option<ParsedForOfStatement> {
    let binding_name = match &for_of_statement.left {
        ForStatementLeft::VariableDeclaration(declaration) => {
            let declarator = declaration.declarations.first()?;
            parse_binding_name(&declarator.id)
        }
        ForStatementLeft::AssignmentTargetIdentifier(identifier) => ParsedBindingName::Identifier {
            name: identifier.name.to_string(),
            span: Some(text_span_from_oxc_span(identifier.span)),
        },
        _ => {
            return None;
        }
    };

    let (iterable, iterable_span) = parse_expression(&for_of_statement.right);
    let body = parse_branch_body(&for_of_statement.body);

    Some(ParsedForOfStatement {
        binding_name,
        iterable,
        iterable_span: Some(text_span_from_oxc_span(iterable_span)),
        body,
    })
}

fn parse_switch_statement(switch_statement: &SwitchStatement<'_>) -> Option<ParsedSwitchStatement> {
    let (discriminant, discriminant_span) = parse_expression(&switch_statement.discriminant);
    let cases = switch_statement
        .cases
        .iter()
        .map(parse_switch_case)
        .collect::<Option<Vec<_>>>()?;

    Some(ParsedSwitchStatement {
        discriminant,
        discriminant_span: Some(text_span_from_oxc_span(discriminant_span)),
        cases,
        span: Some(text_span_from_oxc_span(switch_statement.span)),
    })
}

fn parse_switch_case(switch_case: &SwitchCase<'_>) -> Option<ParsedSwitchCase> {
    let test = switch_case
        .test
        .as_ref()
        .map(|expression| parse_expression(expression).0);
    let test_span = switch_case
        .test
        .as_ref()
        .map(|expression| text_span_from_oxc_span(expression.span()));

    Some(ParsedSwitchCase {
        test,
        test_span,
        consequent: parse_statement_list_as_function_body(&switch_case.consequent),
        span: Some(text_span_from_oxc_span(switch_case.span)),
    })
}

fn parse_throw_statement(throw_statement: &ThrowStatement<'_>) -> Option<ParsedThrowStatement> {
    let (expression, expression_span) = parse_expression(&throw_statement.argument);

    Some(ParsedThrowStatement {
        expression,
        expression_span: Some(text_span_from_oxc_span(expression_span)),
        span: Some(text_span_from_oxc_span(throw_statement.span)),
    })
}

fn parse_try_statement(try_statement: &TryStatement<'_>) -> Option<ParsedTryStatement> {
    let block = parse_block_statement_as_function_body(&try_statement.block);
    let handler = try_statement
        .handler
        .as_ref()
        .map(|catch_clause| parse_catch_clause(catch_clause.as_ref()));
    let finalizer = try_statement
        .finalizer
        .as_ref()
        .map(|finalizer| parse_block_statement_as_function_body(finalizer))
        .unwrap_or_default();

    Some(ParsedTryStatement {
        block,
        handler,
        finalizer,
        span: Some(text_span_from_oxc_span(try_statement.span)),
    })
}

fn parse_catch_clause(catch_clause: &CatchClause<'_>) -> crate::ParsedCatchClause {
    let declared_type = catch_clause
        .param
        .as_ref()
        .and_then(|param| param.type_annotation.as_ref())
        .and_then(|annotation| parse_type_annotation(annotation));
    let binding_name = catch_clause
        .param
        .as_ref()
        .map(|param| parse_binding_name(&param.pattern));

    crate::ParsedCatchClause {
        binding_name,
        declared_type,
        body: parse_block_statement_as_function_body(&catch_clause.body),
        span: Some(text_span_from_oxc_span(catch_clause.span)),
    }
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
    let initializer = parameter.initializer.as_ref().map(|expression| {
        let (parsed_expression, _) = parse_expression(expression);
        parsed_expression
    });
    let initializer_span = parameter
        .initializer
        .as_ref()
        .map(|initializer| text_span_from_oxc_span(initializer.span()));

    Some(ParsedFunctionParameter {
        binding_name: parse_binding_name(&parameter.pattern),
        declared_type,
        initializer,
        initializer_span,
        optional: parameter.optional || parameter.initializer.is_some(),
        rest: false,
        is_parameter_property: parameter.accessibility.is_some() || parameter.readonly,
    })
}

pub(crate) fn parse_rest_function_parameter(
    rest: &oxc_ast::ast::FormalParameterRest<'_>,
) -> Option<ParsedFunctionParameter> {
    let declared_type = rest
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation));

    Some(ParsedFunctionParameter {
        binding_name: parse_binding_name(&rest.rest.argument),
        declared_type,
        initializer: None,
        initializer_span: None,
        optional: false,
        rest: true,
        is_parameter_property: false,
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
        BindingPattern::ArrayPattern(array_pattern) => {
            ParsedBindingName::ArrayPattern(parse_array_binding_pattern(array_pattern))
        }
    }
}

fn parse_array_binding_pattern(
    array_pattern: &oxc_ast::ast::ArrayPattern<'_>,
) -> ParsedArrayBindingPattern {
    ParsedArrayBindingPattern {
        elements: array_pattern
            .elements
            .iter()
            .map(|element| element.as_ref().map(|element| parse_binding_name(element)))
            .collect(),
        rest: array_pattern
            .rest
            .as_deref()
            .map(|rest| Box::new(parse_binding_name(&rest.argument))),
        span: Some(text_span_from_oxc_span(array_pattern.span)),
    }
}

fn parse_object_binding_pattern(object_pattern: &ObjectPattern<'_>) -> ParsedObjectBindingPattern {
    ParsedObjectBindingPattern {
        elements: object_pattern
            .properties
            .iter()
            .filter_map(parse_object_binding_element)
            .collect(),
        rest: object_pattern
            .rest
            .as_deref()
            .map(|rest| Box::new(parse_binding_name(&rest.argument))),
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
        ParsedBindingName::ArrayPattern(pattern) => pattern.span,
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
