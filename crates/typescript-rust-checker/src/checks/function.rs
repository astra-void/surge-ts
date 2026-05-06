use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedArrowFunction, ParsedArrowFunctionBody, ParsedAssignment, ParsedBindingName,
    ParsedExpression, ParsedFunctionBodyStatement, ParsedFunctionDeclaration,
    ParsedFunctionParameter, ParsedIfStatement, ParsedObjectBindingElement,
    ParsedObjectBindingPattern, ParsedReturnStatement, ParsedType, ParsedVariableDeclaration,
    ParsedVariableKind, ParsedWhileStatement,
};
use typescript_rust_types::{FunctionType, Type, is_assignable_to};

use super::assign::check_assignment_with_symbols;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use super::expr::evaluate_expression;
use super::var::{VariableCheckOptions, check_variable_declaration_with_symbols};
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    AssignmentState, FunctionFlowState, analyze_function_body_flow,
    apply_variable_declaration_state, check_assignment_target_flow, check_expression_flow,
    check_obvious_truthiness_condition, collect_future_block_scoped_declarations,
    mark_assignment_state, merge_branch_states,
};
use crate::infer::{
    InferredExpression, TypeParameterSubstitution, map_parsed_type_with_substitution,
    report_duplicate_type_parameters,
};
use crate::symbols::{ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

fn emit_parameter_diagnostics(
    parameter: &ParsedFunctionParameter,
    contextual_type: Option<&Type>,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_implicit_any || parameter.declared_type.is_some() {
        return;
    }

    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, span } => {
            if contextual_type.is_some() {
                return;
            }
            let diagnostic = Diagnostic::ts7006(name, ctx.file_name.clone());
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(*span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            if contextual_type.is_some() {
                return;
            }
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

fn emit_object_binding_pattern_diagnostics(
    pattern: &ParsedObjectBindingPattern,
    ctx: &mut CheckerContext,
) {
    for element in &pattern.elements {
        emit_object_binding_element_diagnostic(element, ctx);
    }
}

fn emit_object_binding_element_diagnostic(
    element: &ParsedObjectBindingElement,
    ctx: &mut CheckerContext,
) {
    match &element.binding_name {
        ParsedBindingName::Identifier { name, span } => {
            let diagnostic = Diagnostic::ts7031(name, "any", ctx.file_name.clone());
            let span = (*span).or(element.name_span);
            let diagnostic = match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            emit_object_binding_pattern_diagnostics(pattern, ctx);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

fn parameter_scope_type(parameter: &ParsedFunctionParameter, parameter_type: &Type) -> Type {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { .. } => parameter_type.clone(),
        ParsedBindingName::ObjectPattern(_) | ParsedBindingName::Unsupported { .. } => Type::Any,
    }
}

fn insert_parameter_bindings(
    parameter: &ParsedFunctionParameter,
    parameter_type: &Type,
    scopes: &mut ScopeStack,
) {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.clone(),
                SymbolInfo {
                    ty: parameter_scope_type(parameter, parameter_type),
                    kind: SymbolKind::Parameter,
                },
            );
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            insert_object_binding_pattern_bindings(
                pattern,
                parameter_scope_type(parameter, parameter_type),
                scopes,
            );
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

fn insert_object_binding_pattern_bindings(
    pattern: &ParsedObjectBindingPattern,
    parameter_type: Type,
    scopes: &mut ScopeStack,
) {
    for element in &pattern.elements {
        insert_object_binding_element_binding(element, parameter_type.clone(), scopes);
    }
}

fn insert_object_binding_element_binding(
    element: &ParsedObjectBindingElement,
    parameter_type: Type,
    scopes: &mut ScopeStack,
) {
    match &element.binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.clone(),
                SymbolInfo {
                    ty: parameter_type,
                    kind: SymbolKind::Parameter,
                },
            );
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            insert_object_binding_pattern_bindings(pattern, parameter_type, scopes);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

fn map_function_signature(
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
    type_parameters: &[typescript_rust_syntax::ParsedTypeParameter],
    contextual_parameter_types: Option<&[Type]>,
    ctx: &mut CheckerContext,
) -> FunctionType {
    report_duplicate_type_parameters(type_parameters, ctx);

    let type_parameter_substitution = build_type_parameter_substitution(type_parameters);

    let parameter_types = parameters
        .iter()
        .map(|parameter| {
            parameter
                .declared_type
                .clone()
                .map_or(Type::Any, |declared_type| {
                    map_parsed_type_with_substitution(
                        declared_type,
                        ctx,
                        &type_parameter_substitution,
                    )
                })
        })
        .collect::<Vec<_>>();

    let function_return_type = return_type
        .map(|return_type| {
            map_parsed_type_with_substitution(
                return_type.clone(),
                ctx,
                &type_parameter_substitution,
            )
        })
        .unwrap_or(Type::Unknown);

    if ctx.options.no_implicit_any {
        for (index, parameter) in parameters.iter().enumerate() {
            let contextual_type = contextual_parameter_types.and_then(|types| types.get(index));
            emit_parameter_diagnostics(parameter, contextual_type, ctx);
        }
    }

    FunctionType {
        parameters: parameter_types,
        return_type: Box::new(function_return_type),
        is_variadic: false,
    }
}

fn has_contextual_unknown_object_binding_pattern(
    parameters: &[ParsedFunctionParameter],
    contextual_parameter_types: Option<&[Type]>,
) -> bool {
    let Some(contextual_parameter_types) = contextual_parameter_types else {
        return false;
    };

    parameters.iter().enumerate().any(|(index, parameter)| {
        matches!(parameter.binding_name, ParsedBindingName::ObjectPattern(_))
            && parameter.declared_type.is_none()
            && contextual_parameter_types
                .get(index)
                .is_some_and(|ty| *ty == Type::Unknown)
    })
}

fn build_type_parameter_substitution(
    type_parameters: &[typescript_rust_syntax::ParsedTypeParameter],
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();

    for type_parameter in type_parameters {
        substitution
            .entry(type_parameter.name.clone())
            .or_insert(Type::Unknown);
    }

    substitution
}

fn register_function_signature(
    name: String,
    function_type: FunctionType,
    symbols: &mut SymbolTable,
    replace_existing: bool,
) -> bool {
    let duplicate = matches!(
        symbols.get(&name),
        Some(existing) if matches!(existing.kind, SymbolKind::Function)
    );

    if duplicate && !replace_existing {
        return true;
    }

    if !duplicate || replace_existing {
        symbols.insert(
            name,
            SymbolInfo {
                ty: Type::Function(function_type),
                kind: SymbolKind::Function,
            },
        );
    }

    duplicate
}

fn check_function_body_with_signature(
    name: String,
    parameters: Vec<ParsedFunctionParameter>,
    body: Vec<ParsedFunctionBodyStatement>,
    function_type: &FunctionType,
    ctx: &mut CheckerContext,
) {
    let body_flow = analyze_function_body_flow(&body);

    let mut scopes = ScopeStack::from_root(ctx.symbols.clone());
    scopes.insert_current(
        name,
        SymbolInfo {
            ty: Type::Function(function_type.clone()),
            kind: SymbolKind::Function,
        },
    );
    scopes.push_child();
    let mut flow_state = FunctionFlowState::new();

    for (parameter, parameter_type) in parameters
        .into_iter()
        .zip(function_type.parameters.iter().cloned())
    {
        insert_parameter_bindings(&parameter, &parameter_type, &mut scopes);
    }

    check_function_body(
        body,
        Some((*function_type.return_type).clone()),
        &mut scopes,
        &mut flow_state,
        ctx,
    );

    if should_check_missing_return(function_type.return_type.as_ref()) {
        emit_missing_return_diagnostic(body_flow, ctx);
    }
}

pub(crate) fn collect_function_declaration_signature(
    function: &ParsedFunctionDeclaration,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) -> FunctionType {
    let temp_symbols = std::mem::take(symbols);
    ctx.set_symbols(temp_symbols);

    let FunctionType {
        parameters,
        return_type,
        is_variadic: _,
    } = map_function_signature(
        &function.parameters,
        function.return_type.as_ref(),
        &function.type_parameters,
        None,
        ctx,
    );

    *symbols = std::mem::take(&mut ctx.symbols);

    let function_type = FunctionType {
        parameters,
        return_type,
        is_variadic: false,
    };

    let duplicate =
        register_function_signature(function.name.clone(), function_type.clone(), symbols, false);

    if duplicate {
        let diagnostic = Diagnostic::ts2393(ctx.file_name.clone());
        let diagnostic = match function.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };

        ctx.push(diagnostic);
    }

    function_type
}

pub(crate) fn check_function_declaration(
    function: ParsedFunctionDeclaration,
    ctx: &mut CheckerContext,
) {
    let ParsedFunctionDeclaration {
        name,
        name_span,
        type_parameters,
        parameters,
        return_type,
        body,
        ..
    } = function;

    let function_type = map_function_signature(
        &parameters,
        return_type.as_ref(),
        &type_parameters,
        None,
        ctx,
    );

    let duplicate = {
        let symbols = &mut ctx.symbols;
        register_function_signature(name.clone(), function_type.clone(), symbols, true)
    };

    if duplicate {
        let diagnostic = Diagnostic::ts2393(ctx.file_name.clone());
        let diagnostic = match name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };

        ctx.push(diagnostic);
    }

    check_function_body_with_signature(name, parameters, body, &function_type, ctx);
}

pub(crate) fn check_function_declaration_body(
    function: ParsedFunctionDeclaration,
    function_type: &FunctionType,
    ctx: &mut CheckerContext,
) {
    let ParsedFunctionDeclaration {
        name,
        parameters,
        body,
        ..
    } = function;

    check_function_body_with_signature(name, parameters, body, function_type, ctx);
}

pub(crate) fn check_arrow_function_expression(
    arrow: ParsedArrowFunction,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> FunctionType {
    check_arrow_function_expression_with_expected_type(arrow, None, symbols, ctx)
}

pub(crate) fn check_arrow_function_expression_with_expected_type(
    arrow: ParsedArrowFunction,
    expected_type: Option<&FunctionType>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> FunctionType {
    let ParsedArrowFunction {
        type_parameters,
        parameters,
        return_type,
        body,
        span: arrow_span,
    } = arrow;

    let contextual_parameter_types =
        expected_type.map(|expected_type| expected_type.parameters.as_slice());
    let mut function_type = map_function_signature(
        &parameters,
        return_type.as_ref(),
        &type_parameters,
        contextual_parameter_types,
        ctx,
    );
    let raw_function_type = function_type.clone();
    let has_explicit_return_type = return_type.is_some();

    if let Some(expected_type) = expected_type {
        for (index, parameter_type) in expected_type.parameters.iter().cloned().enumerate() {
            if index < function_type.parameters.len() && parameters[index].declared_type.is_none() {
                function_type.parameters[index] = parameter_type;
            }
        }

        if has_contextual_unknown_object_binding_pattern(&parameters, contextual_parameter_types) {
            let source_type_name = Type::Function(raw_function_type).name();
            let target_type_name = Type::Function(expected_type.clone()).name();
            let diagnostic =
                Diagnostic::ts2345(&source_type_name, &target_type_name, ctx.file_name.clone());
            let diagnostic = match arrow_span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
        }
    }

    let mut scopes = ScopeStack::from_root(symbols.clone());
    scopes.push_child();
    for (index, parameter) in parameters.iter().enumerate() {
        let parameter_type = function_type
            .parameters
            .get(index)
            .cloned()
            .unwrap_or(Type::Any);
        insert_parameter_bindings(parameter, &parameter_type, &mut scopes);
    }

    let visible_symbols = visible_symbols(&scopes);
    match body {
        ParsedArrowFunctionBody::Expression(expression) => {
            let return_type = (*function_type.return_type).clone();
            let inferred_body = match return_type {
                Type::Any | Type::Unknown | Type::Void => {
                    evaluate_expression(&expression, None, &visible_symbols, ctx)
                }
                _ => evaluate_expression_with_expected_type(
                    &expression,
                    None,
                    Some(&return_type),
                    ExpectedTypeDiagnostic::TypeNotAssignable,
                    &visible_symbols,
                    ctx,
                ),
            };

            if !has_explicit_return_type {
                if let InferredExpression::Known(body_type) = inferred_body {
                    if body_type != Type::Unknown {
                        function_type.return_type = Box::new(body_type);
                    }
                }
            }
        }
        ParsedArrowFunctionBody::Block(statements) => {
            let mut flow_state = FunctionFlowState::new();
            let return_type = match function_type.return_type.as_ref() {
                Type::Any | Type::Unknown | Type::Void => None,
                ty => Some(ty.clone()),
            };
            check_function_body(statements, return_type, &mut scopes, &mut flow_state, ctx);
        }
    }

    function_type
}

fn should_check_missing_return(return_type: &Type) -> bool {
    !matches!(
        return_type,
        Type::Any | Type::Unknown | Type::Undefined | Type::Void
    )
}

fn emit_missing_return_diagnostic(
    body_flow: crate::flow::FunctionBodyFlow,
    ctx: &mut CheckerContext,
) {
    if body_flow.contains_value_return {
        if !body_flow.guarantees_value_return {
            ctx.push(Diagnostic::ts2366(ctx.file_name.clone()));
        }
    } else {
        ctx.push(Diagnostic::ts2355(ctx.file_name.clone()));
    }
}

fn check_function_body(
    body: Vec<ParsedFunctionBodyStatement>,
    return_type: Option<Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let future_block_scoped_declarations = collect_future_block_scoped_declarations(&body);
    flow_state.push_scope(future_block_scoped_declarations);

    for (statement_index, statement) in body.into_iter().enumerate() {
        check_function_body_statement(
            statement,
            statement_index,
            return_type.clone(),
            scopes,
            flow_state,
            ctx,
        );
    }

    flow_state.pop_scope();
}

fn check_function_body_statement(
    statement: ParsedFunctionBodyStatement,
    statement_index: usize,
    return_type: Option<Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
            check_function_variable_declaration(variable, statement_index, scopes, flow_state, ctx);
        }
        ParsedFunctionBodyStatement::Block(block_body) => {
            check_function_block(block_body, return_type, scopes, flow_state, ctx);
        }
        ParsedFunctionBodyStatement::Return(return_statement) => {
            let visible_symbols = visible_symbols(scopes);
            check_function_return_statement(
                return_statement,
                statement_index,
                return_type,
                flow_state,
                &visible_symbols,
                ctx,
            );
        }
        ParsedFunctionBodyStatement::Assignment(assignment) => {
            check_function_assignment(assignment, statement_index, scopes, flow_state, ctx);
        }
        ParsedFunctionBodyStatement::Expression(expression) => {
            check_function_expression_statement(
                expression,
                statement_index,
                scopes,
                flow_state,
                ctx,
            );
        }
        ParsedFunctionBodyStatement::If(if_statement) => {
            check_function_if_statement(
                if_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            check_function_while_statement(
                while_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
        }
    }
}

fn check_function_variable_declaration(
    variable: ParsedVariableDeclaration,
    statement_index: usize,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let local_name = variable.name.clone();
    let variable_kind = variable.kind;
    let has_initializer = variable.initializer.is_some();

    check_local_duplicate_declaration(&variable, scopes, ctx);

    let initializer_flow_blocked = variable.initializer.as_ref().is_some_and(|initializer| {
        let mut initializer_flow_state = flow_state.clone();
        if matches!(
            variable_kind,
            ParsedVariableKind::Let | ParsedVariableKind::Const
        ) {
            initializer_flow_state
                .declare_current(local_name.clone(), AssignmentState::DeclaredUnassigned);
        }

        check_expression_flow(
            initializer,
            variable.initializer_span,
            &initializer_flow_state,
            statement_index,
            ctx,
        )
        .is_blocked()
    });

    let mut visible_symbols = visible_symbols(scopes);

    if let Some(symbol) = check_variable_declaration_with_symbols(
        variable,
        &mut visible_symbols,
        ctx,
        VariableCheckOptions {
            report_duplicate_let_const: false,
            check_initializer: !initializer_flow_blocked,
        },
    ) {
        scopes.insert_current(local_name.clone(), symbol);
        apply_variable_declaration_state(variable_kind, local_name, has_initializer, flow_state);
    }
}

fn check_function_block(
    block_body: Vec<ParsedFunctionBodyStatement>,
    return_type: Option<Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    scopes.push_child();
    check_function_body(block_body, return_type, scopes, flow_state, ctx);
    scopes.pop_child();
}

fn check_function_if_statement(
    if_statement: ParsedIfStatement,
    statement_index: usize,
    return_type: Option<Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    check_obvious_truthiness_condition(&if_statement.condition, if_statement.condition_span, ctx);

    let condition_blocked = check_expression_flow(
        &if_statement.condition,
        if_statement.condition_span,
        flow_state,
        statement_index,
        ctx,
    );

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_expression(
            &if_statement.condition,
            if_statement.condition_span,
            &visible_symbols,
            ctx,
        );
    }

    let base_flow_state = flow_state.clone();

    let mut then_flow_state = base_flow_state.clone();
    scopes.push_child();
    check_function_body(
        if_statement.then_body,
        return_type.clone(),
        scopes,
        &mut then_flow_state,
        ctx,
    );
    scopes.pop_child();

    let mut branch_states = vec![then_flow_state];

    if !if_statement.else_body.is_empty() {
        let mut else_flow_state = base_flow_state.clone();
        scopes.push_child();
        check_function_body(
            if_statement.else_body,
            return_type,
            scopes,
            &mut else_flow_state,
            ctx,
        );
        scopes.pop_child();
        branch_states.push(else_flow_state);
    }

    *flow_state = merge_branch_states(&base_flow_state, &branch_states);
}

fn check_function_while_statement(
    while_statement: ParsedWhileStatement,
    statement_index: usize,
    return_type: Option<Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    check_obvious_truthiness_condition(
        &while_statement.condition,
        while_statement.condition_span,
        ctx,
    );

    let condition_blocked = check_expression_flow(
        &while_statement.condition,
        while_statement.condition_span,
        flow_state,
        statement_index,
        ctx,
    );

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_expression(
            &while_statement.condition,
            while_statement.condition_span,
            &visible_symbols,
            ctx,
        );
    }

    let mut body_flow_state = flow_state.clone();
    scopes.push_child();
    check_function_body(
        while_statement.body,
        return_type,
        scopes,
        &mut body_flow_state,
        ctx,
    );
    scopes.pop_child();
}

fn check_function_assignment(
    assignment: ParsedAssignment,
    statement_index: usize,
    scopes: &ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let target_name = assignment.target_name.clone();

    let target_blocked = check_assignment_target_flow(
        &target_name,
        flow_state,
        statement_index,
        ctx,
        assignment.target_span,
    );

    let value_blocked = check_expression_flow(
        &assignment.value,
        assignment.value_span,
        flow_state,
        statement_index,
        ctx,
    );

    if !target_blocked.is_blocked() && !value_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        check_assignment_with_symbols(assignment, &visible_symbols, ctx);
    }

    if !target_blocked.is_blocked() {
        mark_assignment_state(&target_name, flow_state);
    }
}

fn check_function_expression_statement(
    expression: ParsedExpression,
    statement_index: usize,
    scopes: &ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    if let ParsedExpression::Conditional {
        condition,
        condition_span,
        ..
    } = &expression
    {
        check_obvious_truthiness_condition(condition, *condition_span, ctx);
    }

    let flow_blocked = check_expression_flow(&expression, None, flow_state, statement_index, ctx);

    if flow_blocked.is_blocked() {
        return;
    }

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
    statement_index: usize,
    return_type: Option<Type>,
    flow_state: &mut FunctionFlowState,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(expression) = return_statement.expression.as_ref() else {
        return;
    };

    let flow_blocked = check_expression_flow(
        expression,
        return_statement.expression_span,
        flow_state,
        statement_index,
        ctx,
    );

    if flow_blocked.is_blocked() {
        return;
    }

    let Some(return_type) = return_type else {
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
