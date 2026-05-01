use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedAssignment, ParsedExpression, ParsedFunctionBodyStatement, ParsedFunctionDeclaration,
    ParsedIfStatement, ParsedReturnStatement, ParsedType, ParsedVariableDeclaration,
    ParsedVariableKind, ParsedWhileStatement,
};
use typescript_rust_types::{FunctionType, Type, is_assignable_to};

use crate::check_assign::check_assignment_with_symbols;
use crate::check_expr::{
    ExpectedTypeDiagnostic, evaluate_expression, evaluate_expression_with_expected_type,
};
use crate::check_var::{VariableCheckOptions, check_variable_declaration_with_symbols};
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::infer::{InferredExpression, map_parsed_type};
use crate::symbols::{ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

pub(crate) fn check_function_declaration(
    function: ParsedFunctionDeclaration,
    ctx: &mut CheckerContext,
) {
    let ParsedFunctionDeclaration {
        name,
        parameters,
        return_type,
        body,
        ..
    } = function;

    let parameter_types = parameters
        .iter()
        .map(|parameter| {
            parameter
                .declared_type
                .map_or(Type::Unknown, map_parsed_type)
        })
        .collect::<Vec<_>>();

    let function_return_type = return_type
        .as_ref()
        .map(|return_type| map_parsed_type(return_type.clone()))
        .unwrap_or(Type::Unknown);

    if ctx.options.no_implicit_any {
        for parameter in &parameters {
            if parameter.declared_type.is_none() {
                let diagnostic = Diagnostic::ts7006(&parameter.name, ctx.file_name.clone());
                let diagnostic = match parameter.name_span {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };

                ctx.push(diagnostic);
            }
        }
    }

    ctx.symbols.insert(
        name,
        SymbolInfo {
            ty: Type::Function(FunctionType {
                parameters: parameter_types.clone(),
                return_type: Box::new(function_return_type.clone()),
            }),
            kind: SymbolKind::Function,
        },
    );

    let mut scopes = ScopeStack::from_root(ctx.symbols.clone());
    scopes.push_child();

    for (parameter, parameter_type) in parameters.into_iter().zip(parameter_types.into_iter()) {
        scopes.insert_current(
            parameter.name,
            SymbolInfo {
                ty: parameter_type,
                kind: SymbolKind::Parameter,
            },
        );
    }

    check_function_body(body, return_type, &mut scopes, ctx);
}

fn check_function_body(
    body: Vec<ParsedFunctionBodyStatement>,
    return_type: Option<ParsedType>,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    for statement in body {
        check_function_body_statement(statement, return_type, scopes, ctx);
    }
}

fn check_function_body_statement(
    statement: ParsedFunctionBodyStatement,
    return_type: Option<ParsedType>,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
            check_function_variable_declaration(variable, scopes, ctx);
        }
        ParsedFunctionBodyStatement::Block(block_body) => {
            check_function_block(block_body, return_type, scopes, ctx);
        }
        ParsedFunctionBodyStatement::Return(return_statement) => {
            let visible_symbols = visible_symbols(scopes);
            check_function_return_statement(return_statement, return_type, &visible_symbols, ctx);
        }
        ParsedFunctionBodyStatement::Assignment(assignment) => {
            check_function_assignment(assignment, scopes, ctx);
        }
        ParsedFunctionBodyStatement::Expression(expression) => {
            check_function_expression_statement(expression, scopes, ctx);
        }
        ParsedFunctionBodyStatement::If(if_statement) => {
            check_function_if_statement(if_statement, return_type, scopes, ctx);
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            check_function_while_statement(while_statement, return_type, scopes, ctx);
        }
    }
}

fn check_function_variable_declaration(
    variable: ParsedVariableDeclaration,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let local_name = variable.name.clone();

    check_local_duplicate_declaration(&variable, scopes, ctx);

    let mut visible_symbols = visible_symbols(scopes);

    if let Some(symbol) = check_variable_declaration_with_symbols(
        variable,
        &mut visible_symbols,
        ctx,
        VariableCheckOptions {
            report_duplicate_let_const: false,
        },
    ) {
        scopes.insert_current(local_name, symbol);
    }
}

fn check_function_block(
    block_body: Vec<ParsedFunctionBodyStatement>,
    return_type: Option<ParsedType>,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    scopes.push_child();
    check_function_body(block_body, return_type, scopes, ctx);
    scopes.pop_child();
}

fn check_function_if_statement(
    if_statement: ParsedIfStatement,
    return_type: Option<ParsedType>,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let visible_symbols = visible_symbols(scopes);
    let _ = evaluate_expression(
        &if_statement.condition,
        if_statement.condition_span,
        &visible_symbols,
        ctx,
    );

    scopes.push_child();
    check_function_body(if_statement.then_body, return_type, scopes, ctx);
    scopes.pop_child();

    if !if_statement.else_body.is_empty() {
        scopes.push_child();
        check_function_body(if_statement.else_body, return_type, scopes, ctx);
        scopes.pop_child();
    }
}

fn check_function_while_statement(
    while_statement: ParsedWhileStatement,
    return_type: Option<ParsedType>,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let visible_symbols = visible_symbols(scopes);
    let _ = evaluate_expression(
        &while_statement.condition,
        while_statement.condition_span,
        &visible_symbols,
        ctx,
    );

    scopes.push_child();
    check_function_body(while_statement.body, return_type, scopes, ctx);
    scopes.pop_child();
}

fn check_function_assignment(
    assignment: ParsedAssignment,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    let visible_symbols = visible_symbols(scopes);
    check_assignment_with_symbols(assignment, &visible_symbols, ctx);
}

fn check_function_expression_statement(
    expression: ParsedExpression,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    let visible_symbols = visible_symbols(scopes);
    let _ = evaluate_expression(&expression, None, &visible_symbols, ctx);
}

fn visible_symbols(scopes: &ScopeStack) -> SymbolTable {
    scopes.visible_symbols()
}

fn check_local_duplicate_declaration(
    variable: &ParsedVariableDeclaration,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) {
    if matches!(
        variable.kind,
        ParsedVariableKind::Let | ParsedVariableKind::Const
    ) && scopes.current_contains_let_or_const(&variable.name)
    {
        let diagnostic = Diagnostic::ts2451(&variable.name, ctx.file_name.clone());
        let diagnostic = match variable.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };

        ctx.push(diagnostic);
    }
}

fn check_function_return_statement(
    return_statement: ParsedReturnStatement,
    return_type: Option<ParsedType>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(return_type) = return_type.map(map_parsed_type) else {
        return;
    };

    let Some(expression) = return_statement.expression.as_ref() else {
        return;
    };

    let inferred_expression = evaluate_expression_with_expected_type(
        expression,
        return_statement.expression_span,
        Some(&return_type),
        ExpectedTypeDiagnostic::TypeNotAssignable,
        symbols,
        ctx,
    );

    match inferred_expression {
        InferredExpression::Known(source_type) => {
            if source_type == Type::Unknown {
                return;
            }

            if !is_assignable_to(&source_type, &return_type) {
                let source_type_name = source_type.name();
                let target_type_name = return_type.name();
                let diagnostic =
                    Diagnostic::ts2322(&source_type_name, &target_type_name, ctx.file_name.clone());

                let diagnostic = match return_statement.expression_span {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };

                ctx.push(diagnostic);
            }
        }
        InferredExpression::UnresolvedIdentifier { .. } => {}
        InferredExpression::MissingProperty { .. } => {}
        InferredExpression::Unknown => {}
    }
}
