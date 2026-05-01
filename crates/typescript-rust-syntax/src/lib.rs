use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, AssignmentOperator, AssignmentTarget, BinaryExpression, BinaryOperator,
    BindingPattern, BlockStatement, ConditionalExpression, Declaration, Expression,
    ExpressionStatement, FormalParameter, Function, IfStatement, LogicalExpression,
    LogicalOperator, ObjectExpression, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    TSType, UnaryExpression, UnaryOperator, VariableDeclaration, VariableDeclarationKind,
    WhileStatement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

#[derive(Debug, Clone)]
pub struct ParsedSource {
    pub file_name: String,
    pub statements: Vec<ParsedStatement>,
    pub parser_errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ParsedStatement {
    VariableDeclaration(ParsedVariableDeclaration),
    Assignment(ParsedAssignment),
    FunctionDeclaration(ParsedFunctionDeclaration),
    Call(ParsedCall),
    Expression(ParsedExpression),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedVariableKind {
    Var,
    Let,
    Const,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedType {
    String,
    Number,
    Boolean,
    Any,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedExpression {
    StringLiteral,
    NumberLiteral,
    BooleanLiteral,
    Identifier(String),
    ObjectLiteral(Vec<ParsedObjectProperty>),
    Unary {
        operator: ParsedUnaryOperator,
        operator_span: Option<TextSpan>,
        operand: Box<ParsedExpression>,
        operand_span: Option<TextSpan>,
    },
    Binary {
        left: Box<ParsedExpression>,
        left_span: Option<TextSpan>,
        operator: ParsedBinaryOperator,
        operator_span: Option<TextSpan>,
        right: Box<ParsedExpression>,
        right_span: Option<TextSpan>,
    },
    Logical {
        left: Box<ParsedExpression>,
        left_span: Option<TextSpan>,
        operator: ParsedLogicalOperator,
        operator_span: Option<TextSpan>,
        right: Box<ParsedExpression>,
        right_span: Option<TextSpan>,
    },
    Conditional {
        condition: Box<ParsedExpression>,
        condition_span: Option<TextSpan>,
        when_true: Box<ParsedExpression>,
        when_true_span: Option<TextSpan>,
        when_false: Box<ParsedExpression>,
        when_false_span: Option<TextSpan>,
    },
    PropertyAccess {
        object_name: String,
        object_span: Option<TextSpan>,
        property_name: String,
        property_span: Option<TextSpan>,
    },
    Call {
        callee_name: String,
        callee_span: Option<TextSpan>,
        arguments: Vec<ParsedCallArgument>,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedObjectProperty {
    pub name: String,
    pub value: ParsedExpression,
    pub span: Option<TextSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedBinaryOperator {
    StrictEquals,
    StrictNotEquals,
    Equals,
    NotEquals,
    LessThan,
    LessThanEquals,
    GreaterThan,
    GreaterThanEquals,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedLogicalOperator {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedUnaryOperator {
    Not,
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedVariableDeclaration {
    pub kind: ParsedVariableKind,
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub declared_type: Option<ParsedType>,
    pub initializer: Option<ParsedExpression>,
    pub initializer_span: Option<TextSpan>,
}

#[derive(Debug, Clone)]
pub struct ParsedAssignment {
    pub target_name: String,
    pub target_span: Option<TextSpan>,
    pub value: ParsedExpression,
    pub value_span: Option<TextSpan>,
}

#[derive(Debug, Clone)]
pub struct ParsedFunctionDeclaration {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub parameters: Vec<ParsedFunctionParameter>,
    pub return_type: Option<ParsedType>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone)]
pub enum ParsedFunctionBodyStatement {
    VariableDeclaration(ParsedVariableDeclaration),
    Return(ParsedReturnStatement),
    Assignment(ParsedAssignment),
    Expression(ParsedExpression),
    Block(Vec<ParsedFunctionBodyStatement>),
    If(ParsedIfStatement),
    While(ParsedWhileStatement),
}

#[derive(Debug, Clone)]
pub struct ParsedReturnStatement {
    pub expression: Option<ParsedExpression>,
    pub expression_span: Option<TextSpan>,
}

#[derive(Debug, Clone)]
pub struct ParsedIfStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub then_body: Vec<ParsedFunctionBodyStatement>,
    pub else_body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone)]
pub struct ParsedWhileStatement {
    pub condition: ParsedExpression,
    pub condition_span: Option<TextSpan>,
    pub body: Vec<ParsedFunctionBodyStatement>,
}

#[derive(Debug, Clone)]
pub struct ParsedFunctionParameter {
    pub name: String,
    pub name_span: Option<TextSpan>,
    pub declared_type: Option<ParsedType>,
}

#[derive(Debug, Clone)]
pub struct ParsedCall {
    pub callee_name: String,
    pub callee_span: Option<TextSpan>,
    pub arguments: Vec<ParsedCallArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCallArgument {
    pub expression: ParsedExpression,
    pub span: Option<TextSpan>,
}

pub fn parse_source(source_text: &str, file_name: &str) -> ParsedSource {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser = Parser::new(&allocator, source_text, source_type);
    let parsed = parser.parse();

    let statements = parsed
        .program
        .body
        .iter()
        .filter_map(parse_statement)
        .flatten()
        .collect();

    let parser_errors = parsed
        .errors
        .into_iter()
        .map(|error| error.to_string())
        .collect();

    ParsedSource {
        file_name: file_name.to_string(),
        statements,
        parser_errors,
    }
}

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
        Expression::CallExpression(call_expression) => {
            parse_call_expression(call_expression).map(ParsedStatement::Call)
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

fn parse_function_declaration(function: &Function<'_>) -> Option<ParsedFunctionDeclaration> {
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
        .and_then(|annotation| parse_type_annotation(annotation))
        .and_then(|parsed_type| match parsed_type {
            ParsedType::String | ParsedType::Number | ParsedType::Boolean => Some(parsed_type),
            _ => None,
        });
    let body = function
        .body
        .as_ref()
        .map(|body| parse_statement_list_as_function_body(&body.statements))
        .unwrap_or_default();

    Some(ParsedFunctionDeclaration {
        name: id.name.to_string(),
        name_span: Some(text_span_from_oxc_span(id.span)),
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

fn parse_statement_list_as_function_body(
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
        Expression::AssignmentExpression(assignment) => parse_assignment_expression(assignment)
            .map(|assignment| vec![ParsedFunctionBodyStatement::Assignment(assignment)]),
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
    parse_variable_declaration(declaration)
        .into_iter()
        .filter_map(|statement| match statement {
            ParsedStatement::VariableDeclaration(variable) => {
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

fn parse_function_parameter(parameter: &FormalParameter<'_>) -> Option<ParsedFunctionParameter> {
    let BindingPattern::BindingIdentifier(binding) = &parameter.pattern else {
        return None;
    };

    let declared_type = parameter
        .type_annotation
        .as_ref()
        .and_then(|annotation| parse_type_annotation(annotation));

    Some(ParsedFunctionParameter {
        name: binding.name.to_string(),
        name_span: Some(text_span_from_oxc_span(binding.span)),
        declared_type,
    })
}

fn parse_call_expression(call_expression: &oxc_ast::ast::CallExpression<'_>) -> Option<ParsedCall> {
    let call = parse_call_expression_parts(call_expression)?;

    Some(ParsedCall {
        callee_name: call.callee_name,
        callee_span: call.callee_span,
        arguments: call.arguments,
    })
}

struct ParsedCallExpressionParts {
    callee_name: String,
    callee_span: Option<TextSpan>,
    arguments: Vec<ParsedCallArgument>,
}

fn parse_call_expression_parts(
    call_expression: &oxc_ast::ast::CallExpression<'_>,
) -> Option<ParsedCallExpressionParts> {
    let Expression::Identifier(callee) = &call_expression.callee else {
        return None;
    };

    let arguments = call_expression
        .arguments
        .iter()
        .map(parse_call_argument)
        .collect();

    Some(ParsedCallExpressionParts {
        callee_name: callee.name.to_string(),
        callee_span: Some(text_span_from_oxc_span(callee.span)),
        arguments,
    })
}

fn parse_call_argument(argument: &Argument<'_>) -> ParsedCallArgument {
    let (expression, span) = match argument {
        Argument::SpreadElement(_) => (ParsedExpression::Unknown, argument.span()),
        Argument::BooleanLiteral(_) => (ParsedExpression::BooleanLiteral, argument.span()),
        Argument::NumericLiteral(_) => (ParsedExpression::NumberLiteral, argument.span()),
        Argument::StringLiteral(_) => (ParsedExpression::StringLiteral, argument.span()),
        Argument::Identifier(identifier) => (
            ParsedExpression::Identifier(identifier.name.to_string()),
            argument.span(),
        ),
        Argument::BinaryExpression(binary_expression) => (
            parse_binary_expression(binary_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::LogicalExpression(logical_expression) => (
            parse_logical_expression(logical_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::UnaryExpression(unary_expression) => (
            parse_unary_expression(unary_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::ParenthesizedExpression(parenthesized_expression) => (
            parse_expression(&parenthesized_expression.expression).0,
            argument.span(),
        ),
        Argument::ConditionalExpression(conditional_expression) => (
            parse_conditional_expression(conditional_expression)
                .unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::ObjectExpression(object_expression) => (
            ParsedExpression::ObjectLiteral(parse_object_properties(object_expression)),
            argument.span(),
        ),
        Argument::CallExpression(call_expression) => (
            parse_call_expression_parts(call_expression)
                .map(|call| ParsedExpression::Call {
                    callee_name: call.callee_name,
                    callee_span: call.callee_span,
                    arguments: call.arguments,
                })
                .unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        Argument::StaticMemberExpression(member_expression) => (
            parse_static_member_expression(member_expression).unwrap_or(ParsedExpression::Unknown),
            argument.span(),
        ),
        _ => (ParsedExpression::Unknown, argument.span()),
    };

    ParsedCallArgument {
        expression,
        span: Some(text_span_from_oxc_span(span)),
    }
}

fn parse_type_annotation(
    type_annotation: &oxc_ast::ast::TSTypeAnnotation<'_>,
) -> Option<ParsedType> {
    match &type_annotation.type_annotation {
        TSType::TSStringKeyword(_) => Some(ParsedType::String),
        TSType::TSNumberKeyword(_) => Some(ParsedType::Number),
        TSType::TSBooleanKeyword(_) => Some(ParsedType::Boolean),
        TSType::TSAnyKeyword(_) => Some(ParsedType::Any),
        TSType::TSUnknownKeyword(_) => Some(ParsedType::Unknown),
        _ => None,
    }
}

fn parse_expression(expression: &Expression<'_>) -> (ParsedExpression, Span) {
    let parsed_expression = match expression {
        Expression::StringLiteral(_) => ParsedExpression::StringLiteral,
        Expression::NumericLiteral(_) => ParsedExpression::NumberLiteral,
        Expression::BooleanLiteral(_) => ParsedExpression::BooleanLiteral,
        Expression::Identifier(identifier) => {
            ParsedExpression::Identifier(identifier.name.to_string())
        }
        Expression::ObjectExpression(object_expression) => {
            ParsedExpression::ObjectLiteral(parse_object_properties(object_expression))
        }
        Expression::BinaryExpression(binary_expression) => {
            parse_binary_expression(binary_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::LogicalExpression(logical_expression) => {
            parse_logical_expression(logical_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::UnaryExpression(unary_expression) => {
            parse_unary_expression(unary_expression).unwrap_or(ParsedExpression::Unknown)
        }
        Expression::ParenthesizedExpression(parenthesized_expression) => {
            return parse_expression(&parenthesized_expression.expression);
        }
        Expression::ConditionalExpression(conditional_expression) => {
            parse_conditional_expression(conditional_expression)
                .unwrap_or(ParsedExpression::Unknown)
        }
        Expression::CallExpression(call_expression) => parse_call_expression_parts(call_expression)
            .map(|call| ParsedExpression::Call {
                callee_name: call.callee_name,
                callee_span: call.callee_span,
                arguments: call.arguments,
            })
            .unwrap_or(ParsedExpression::Unknown),
        Expression::StaticMemberExpression(member_expression) => {
            parse_static_member_expression(member_expression).unwrap_or(ParsedExpression::Unknown)
        }
        _ => ParsedExpression::Unknown,
    };

    (parsed_expression, expression.span())
}

fn parse_binary_expression(binary_expression: &BinaryExpression<'_>) -> Option<ParsedExpression> {
    let operator = match binary_expression.operator {
        BinaryOperator::StrictEquality => ParsedBinaryOperator::StrictEquals,
        BinaryOperator::StrictInequality => ParsedBinaryOperator::StrictNotEquals,
        BinaryOperator::Equality => ParsedBinaryOperator::Equals,
        BinaryOperator::Inequality => ParsedBinaryOperator::NotEquals,
        BinaryOperator::LessThan => ParsedBinaryOperator::LessThan,
        BinaryOperator::LessEqualThan => ParsedBinaryOperator::LessThanEquals,
        BinaryOperator::GreaterThan => ParsedBinaryOperator::GreaterThan,
        BinaryOperator::GreaterEqualThan => ParsedBinaryOperator::GreaterThanEquals,
        BinaryOperator::Addition => ParsedBinaryOperator::Add,
        BinaryOperator::Subtraction => ParsedBinaryOperator::Subtract,
        BinaryOperator::Multiplication => ParsedBinaryOperator::Multiply,
        BinaryOperator::Division => ParsedBinaryOperator::Divide,
        BinaryOperator::Remainder => ParsedBinaryOperator::Remainder,
        _ => return None,
    };

    let (left, left_span) = parse_expression(&binary_expression.left);
    let (right, right_span) = parse_expression(&binary_expression.right);

    Some(ParsedExpression::Binary {
        left: Box::new(left),
        left_span: Some(text_span_from_oxc_span(left_span)),
        operator,
        right: Box::new(right),
        right_span: Some(text_span_from_oxc_span(right_span)),
        operator_span: None,
    })
}

fn parse_logical_expression(
    logical_expression: &LogicalExpression<'_>,
) -> Option<ParsedExpression> {
    let operator = match logical_expression.operator {
        LogicalOperator::And => ParsedLogicalOperator::And,
        LogicalOperator::Or => ParsedLogicalOperator::Or,
        LogicalOperator::Coalesce => return None,
    };

    let (left, left_span) = parse_expression(&logical_expression.left);
    let (right, right_span) = parse_expression(&logical_expression.right);

    Some(ParsedExpression::Logical {
        left: Box::new(left),
        left_span: Some(text_span_from_oxc_span(left_span)),
        operator,
        right: Box::new(right),
        right_span: Some(text_span_from_oxc_span(right_span)),
        operator_span: None,
    })
}

fn parse_conditional_expression(
    conditional_expression: &ConditionalExpression<'_>,
) -> Option<ParsedExpression> {
    let (condition, condition_span) = parse_expression(&conditional_expression.test);
    let (when_true, when_true_span) = parse_expression(&conditional_expression.consequent);
    let (when_false, when_false_span) = parse_expression(&conditional_expression.alternate);

    Some(ParsedExpression::Conditional {
        condition: Box::new(condition),
        condition_span: Some(text_span_from_oxc_span(condition_span)),
        when_true: Box::new(when_true),
        when_true_span: Some(text_span_from_oxc_span(when_true_span)),
        when_false: Box::new(when_false),
        when_false_span: Some(text_span_from_oxc_span(when_false_span)),
    })
}

fn parse_unary_expression(unary_expression: &UnaryExpression<'_>) -> Option<ParsedExpression> {
    let operator = match unary_expression.operator {
        UnaryOperator::LogicalNot => ParsedUnaryOperator::Not,
        UnaryOperator::UnaryPlus => ParsedUnaryOperator::Plus,
        UnaryOperator::UnaryNegation => ParsedUnaryOperator::Minus,
        UnaryOperator::BitwiseNot
        | UnaryOperator::Typeof
        | UnaryOperator::Void
        | UnaryOperator::Delete => {
            return Some(ParsedExpression::Unknown);
        }
    };

    let (operand, operand_span) = parse_expression(&unary_expression.argument);

    Some(ParsedExpression::Unary {
        operator,
        operator_span: None,
        operand: Box::new(operand),
        operand_span: Some(text_span_from_oxc_span(operand_span)),
    })
}

fn parse_object_properties(object_expression: &ObjectExpression<'_>) -> Vec<ParsedObjectProperty> {
    object_expression
        .properties
        .iter()
        .filter_map(|property_kind| {
            let ObjectPropertyKind::ObjectProperty(property) = property_kind else {
                return None;
            };

            let PropertyKey::StaticIdentifier(key) = &property.key else {
                return None;
            };

            if property.kind != PropertyKind::Init
                || property.method
                || property.shorthand
                || property.computed
            {
                return None;
            }

            let (value, _) = parse_expression(&property.value);

            Some(ParsedObjectProperty {
                name: key.name.to_string(),
                value,
                span: Some(text_span_from_oxc_span(property.span)),
            })
        })
        .collect()
}

fn parse_static_member_expression(
    member_expression: &oxc_ast::ast::StaticMemberExpression<'_>,
) -> Option<ParsedExpression> {
    let Expression::Identifier(object_identifier) = &member_expression.object else {
        return None;
    };

    Some(ParsedExpression::PropertyAccess {
        object_name: object_identifier.name.to_string(),
        object_span: Some(text_span_from_oxc_span(object_identifier.span)),
        property_name: member_expression.property.name.to_string(),
        property_span: Some(text_span_from_oxc_span(member_expression.property.span)),
    })
}

fn text_span_from_oxc_span(span: Span) -> TextSpan {
    TextSpan {
        start: span.start as usize,
        end: span.end as usize,
    }
}
