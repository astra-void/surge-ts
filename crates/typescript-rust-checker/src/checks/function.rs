use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedArrowFunction, ParsedArrowFunctionBody, ParsedAssignment, ParsedBindingName,
    ParsedExpression, ParsedFunctionBodyStatement, ParsedFunctionDeclaration,
    ParsedFunctionParameter, ParsedIfStatement, ParsedLogicalOperator, ParsedObjectBindingElement,
    ParsedObjectBindingPattern, ParsedReturnStatement, ParsedSwitchStatement, ParsedTryStatement,
    ParsedType, ParsedTypeParameter, ParsedUnaryOperator, ParsedVariableDeclaration,
    ParsedVariableKind, ParsedWhileStatement,
};
use typescript_rust_types::{FunctionType, Type, is_assignable_to};

use super::assign::check_assignment_with_symbols;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use super::expr::evaluate_expression;
use super::var::{
    VariableCheckOptions, check_variable_declaration_with_symbols,
    widen_implicit_variable_initializer_type,
};
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
use crate::symbols::{FunctionSignatureInfo, ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

fn emit_parameter_diagnostics(
    parameter: &ParsedFunctionParameter,
    contextual_type: Option<&Type>,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.no_implicit_any
        || parameter.declared_type.is_some()
        || parameter.initializer.is_some()
    {
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

fn parameter_identifier_name(parameter: &ParsedFunctionParameter) -> Option<&str> {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn parameter_scope_type(parameter: &ParsedFunctionParameter, parameter_type: &Type) -> Type {
    match &parameter.binding_name {
        ParsedBindingName::Identifier { .. } => parameter_type.clone(),
        ParsedBindingName::ObjectPattern(_) | ParsedBindingName::Unsupported { .. } => Type::Any,
    }
}

fn insert_binding_name(binding_name: &ParsedBindingName, ty: Type, scopes: &mut ScopeStack) {
    match binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.clone(),
                SymbolInfo {
                    ty,
                    kind: SymbolKind::Parameter,
                    function_signature: None,
                },
            );
        }
        ParsedBindingName::ObjectPattern(pattern) => {
            insert_object_binding_pattern_bindings(pattern, ty, scopes);
        }
        ParsedBindingName::Unsupported { .. } => {}
    }
}

fn insert_parameter_bindings(
    parameter: &ParsedFunctionParameter,
    parameter_type: &Type,
    scopes: &mut ScopeStack,
) {
    insert_binding_name(
        &parameter.binding_name,
        parameter_scope_type(parameter, parameter_type),
        scopes,
    );
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
                    function_signature: None,
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
    let mut parameter_symbols = ctx.symbols.clone();
    let mut parameter_types = Vec::with_capacity(parameters.len());

    for (index, parameter) in parameters.iter().enumerate() {
        let inferred_parameter_type = if let Some(declared_type) = parameter.declared_type.clone() {
            map_parsed_type_with_substitution(declared_type, ctx, &type_parameter_substitution)
        } else if let Some(initializer) = parameter.initializer.as_ref() {
            let inferred_initializer = evaluate_expression(
                initializer,
                parameter.initializer_span,
                &parameter_symbols,
                ctx,
            );

            match inferred_initializer {
                InferredExpression::Known(ty) => {
                    widen_implicit_variable_initializer_type(SymbolKind::Let, &ty)
                }
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => Type::Unknown,
            }
        } else {
            Type::Any
        };

        if let Some(name) = parameter_identifier_name(parameter) {
            let _ = parameter_symbols.insert(
                name.to_string(),
                SymbolInfo {
                    ty: inferred_parameter_type.clone(),
                    kind: SymbolKind::Parameter,
                    function_signature: None,
                },
            );
        }

        parameter_types.push(inferred_parameter_type);

        if ctx.options.no_implicit_any {
            let contextual_type = contextual_parameter_types.and_then(|types| types.get(index));
            emit_parameter_diagnostics(parameter, contextual_type, ctx);
        }
    }

    let function_return_type = return_type
        .map(|return_type| {
            map_parsed_type_with_substitution(
                return_type.clone(),
                ctx,
                &type_parameter_substitution,
            )
        })
        .unwrap_or(Type::Unknown);

    FunctionType {
        parameters: parameter_types,
        return_type: Box::new(function_return_type),
        is_variadic: false,
        required_parameter_count: required_parameter_count(parameters),
    }
}

fn required_parameter_count(parameters: &[ParsedFunctionParameter]) -> usize {
    let mut required = parameters.len();

    while required > 0 {
        let parameter = &parameters[required - 1];
        if parameter.optional || parameter.initializer.is_some() {
            required -= 1;
        } else {
            break;
        }
    }

    required
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

fn function_signature_info(
    type_parameters: &[ParsedTypeParameter],
    parameters: &[ParsedFunctionParameter],
    return_type: Option<&ParsedType>,
) -> FunctionSignatureInfo {
    FunctionSignatureInfo {
        type_parameters: type_parameters.to_vec(),
        parameter_types: parameters
            .iter()
            .map(|parameter| parameter.declared_type.clone())
            .collect(),
        return_type: return_type.cloned(),
    }
}

fn with_type_parameter_scope<R>(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let mut scope = std::collections::HashMap::new();
    for type_parameter in type_parameters {
        scope.insert(type_parameter.name.clone(), Type::Unknown);
    }

    ctx.push_type_parameter_scope(type_parameters, Some(scope));
    let result = f(ctx);
    ctx.pop_type_parameter_scope();
    result
}

fn register_function_signature(
    name: String,
    function_type: FunctionType,
    function_signature: Option<FunctionSignatureInfo>,
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
                function_signature,
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
    type_parameters: &[ParsedTypeParameter],
    function_signature: Option<FunctionSignatureInfo>,
    has_explicit_return_type: bool,
    ctx: &mut CheckerContext,
) {
    let body_flow = analyze_function_body_flow(&body);

    let mut scopes = ScopeStack::from_root(merged_function_body_root_symbols(ctx));
    scopes.insert_current(
        name,
        SymbolInfo {
            ty: Type::Function(function_type.clone()),
            kind: SymbolKind::Function,
            function_signature,
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

    with_type_parameter_scope(type_parameters, ctx, |ctx| {
        check_function_body(
            body,
            Some((*function_type.return_type).clone()),
            &mut scopes,
            &mut flow_state,
            ctx,
        );
    });

    if has_explicit_return_type && should_check_missing_return(function_type.return_type.as_ref()) {
        emit_missing_return_diagnostic(body_flow, ctx);
    }
}

fn merged_function_body_root_symbols(ctx: &CheckerContext) -> SymbolTable {
    let mut root = ctx.ambient_global_symbols.clone();
    for (name, symbol) in ctx.symbols.iter() {
        root.insert(name.clone(), symbol.clone());
    }

    root
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
        required_parameter_count,
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
        required_parameter_count,
    };

    let duplicate = register_function_signature(
        function.name.clone(),
        function_type.clone(),
        Some(function_signature_info(
            &function.type_parameters,
            &function.parameters,
            function.return_type.as_ref(),
        )),
        symbols,
        false,
    );

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
        is_declare,
        name,
        name_span,
        type_parameters,
        parameters,
        return_type,
        body,
        ..
    } = function;

    with_type_parameter_scope(&type_parameters, ctx, |ctx| {
        let signature_info =
            function_signature_info(&type_parameters, &parameters, return_type.as_ref());
        let function_type = map_function_signature(
            &parameters,
            return_type.as_ref(),
            &type_parameters,
            None,
            ctx,
        );

        let duplicate = {
            let symbols = &mut ctx.symbols;
            register_function_signature(
                name.clone(),
                function_type.clone(),
                Some(signature_info.clone()),
                symbols,
                true,
            )
        };

        if duplicate {
            let diagnostic = Diagnostic::ts2393(ctx.file_name.clone());
            let diagnostic = match name_span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };

            ctx.push(diagnostic);
        }

        if is_declare {
            return;
        }

        check_function_body_with_signature(
            name,
            parameters,
            body,
            &function_type,
            &type_parameters,
            Some(signature_info),
            return_type.is_some(),
            ctx,
        );
    });
}

pub(crate) fn check_function_declaration_body(
    function: ParsedFunctionDeclaration,
    function_type: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) {
    let ParsedFunctionDeclaration {
        is_declare,
        name,
        parameters,
        return_type,
        body,
        ..
    } = function;

    if is_declare {
        return;
    }

    let signature_info =
        function_signature_info(type_parameters, &parameters, return_type.as_ref());
    check_function_body_with_signature(
        name,
        parameters,
        body,
        function_type,
        type_parameters,
        Some(signature_info),
        return_type.is_some(),
        ctx,
    );
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
    with_type_parameter_scope(&type_parameters, ctx, |ctx| {
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
                if index < function_type.parameters.len()
                    && parameters[index].declared_type.is_none()
                {
                    function_type.parameters[index] = parameter_type;
                }
            }

            if has_contextual_unknown_object_binding_pattern(
                &parameters,
                contextual_parameter_types,
            ) {
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

        if expected_type
            .is_some_and(|expected_type| matches!(expected_type.return_type.as_ref(), Type::Void))
            && !has_explicit_return_type
        {
            function_type.return_type = Box::new(Type::Void);
        }

        function_type
    })
}

fn should_check_missing_return(return_type: &Type) -> bool {
    !matches!(
        return_type,
        Type::Any | Type::Unknown | Type::Undefined | Type::Void
    ) && !type_contains_unknown(return_type)
}

fn type_contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Array(element) => type_contains_unknown(element),
        Type::Tuple(elements) => elements.iter().any(type_contains_unknown),
        Type::Function(function) => {
            function.parameters.iter().any(type_contains_unknown)
                || type_contains_unknown(&function.return_type)
        }
        Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| type_contains_unknown(&property.ty))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(type_contains_unknown)
        }
        Type::Union(union) => union.types.iter().any(type_contains_unknown),
        _ => false,
    }
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
        ParsedFunctionBodyStatement::Throw(throw_statement) => {
            check_function_throw_statement(
                throw_statement,
                statement_index,
                scopes,
                flow_state,
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
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            check_function_switch_statement(
                switch_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            check_function_try_statement(
                try_statement,
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

    let then_guarantees_value_return =
        analyze_function_body_flow(&if_statement.then_body).guarantees_value_return;
    let has_else_body = !if_statement.else_body.is_empty();

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

    if has_else_body {
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

    if !has_else_body && then_guarantees_value_return {
        narrow_truthy_guarded_identifiers(&if_statement.condition, scopes);
    }

    *flow_state = merge_branch_states(&base_flow_state, &branch_states);
}

fn narrow_truthy_guarded_identifiers(condition: &ParsedExpression, scopes: &mut ScopeStack) {
    let mut names = Vec::new();
    if !collect_truthy_guarded_identifiers(condition, &mut names) {
        return;
    }

    for name in names {
        let Some(symbol) = scopes.resolve(&name).cloned() else {
            continue;
        };

        let narrowed = typescript_rust_types::remove_undefined(&symbol.ty);
        if narrowed == symbol.ty {
            continue;
        }

        let _ = scopes.update_visible(
            &name,
            SymbolInfo {
                ty: narrowed,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }
}

fn collect_truthy_guarded_identifiers(
    condition: &ParsedExpression,
    names: &mut Vec<String>,
) -> bool {
    match condition {
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } => {
            collect_truthy_guarded_identifiers(left, names)
                && collect_truthy_guarded_identifiers(right, names)
        }
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => match operand.as_ref() {
            ParsedExpression::Identifier { name, .. } => {
                names.push(name.clone());
                true
            }
            _ => false,
        },
        _ => false,
    }
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

fn check_function_switch_statement(
    switch_statement: ParsedSwitchStatement,
    statement_index: usize,
    return_type: Option<Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let condition_blocked = check_expression_flow(
        &switch_statement.discriminant,
        switch_statement.discriminant_span,
        flow_state,
        statement_index,
        ctx,
    );

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_expression(
            &switch_statement.discriminant,
            switch_statement.discriminant_span,
            &visible_symbols,
            ctx,
        );
    }

    let base_flow_state = flow_state.clone();
    let mut branch_states = Vec::new();

    for switch_case in switch_statement.cases {
        let mut case_flow_state = base_flow_state.clone();

        if let Some(test) = switch_case.test.as_ref() {
            let _ = check_expression_flow(
                test,
                switch_case.test_span,
                &case_flow_state,
                statement_index,
                ctx,
            );
        }

        scopes.push_child();
        check_function_body(
            switch_case.consequent,
            return_type.clone(),
            scopes,
            &mut case_flow_state,
            ctx,
        );
        scopes.pop_child();
        branch_states.push(case_flow_state);
    }

    *flow_state = merge_branch_states(&base_flow_state, &branch_states);
}

fn check_function_try_statement(
    try_statement: ParsedTryStatement,
    _statement_index: usize,
    return_type: Option<Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let base_flow_state = flow_state.clone();

    let mut try_flow_state = base_flow_state.clone();
    scopes.push_child();
    check_function_body(
        try_statement.block,
        return_type.clone(),
        scopes,
        &mut try_flow_state,
        ctx,
    );
    scopes.pop_child();

    let mut branch_states = vec![try_flow_state];

    if let Some(handler_clause) = try_statement.handler {
        let mut catch_flow_state = base_flow_state.clone();
        scopes.push_child();
        if let Some(binding_name) = handler_clause.binding_name.as_ref() {
            insert_binding_name(binding_name, Type::Unknown, scopes);
        }
        check_function_body(
            handler_clause.body,
            return_type.clone(),
            scopes,
            &mut catch_flow_state,
            ctx,
        );
        scopes.pop_child();
        branch_states.push(catch_flow_state);
    }

    let mut merged_flow_state = merge_branch_states(&base_flow_state, &branch_states);
    scopes.push_child();
    check_function_body(
        try_statement.finalizer,
        return_type,
        scopes,
        &mut merged_flow_state,
        ctx,
    );
    scopes.pop_child();

    *flow_state = merged_flow_state;
}

fn check_function_throw_statement(
    throw_statement: typescript_rust_syntax::ParsedThrowStatement,
    statement_index: usize,
    scopes: &ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let _ = check_expression_flow(
        &throw_statement.expression,
        throw_statement.expression_span,
        flow_state,
        statement_index,
        ctx,
    );

    let visible_symbols = visible_symbols(scopes);
    let _ = evaluate_expression(
        &throw_statement.expression,
        throw_statement.expression_span,
        &visible_symbols,
        ctx,
    );
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
