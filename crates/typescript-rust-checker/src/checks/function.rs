use std::sync::Arc;
use std::time::Instant;
use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode};
use typescript_rust_syntax::{
    ParsedArrowFunction, ParsedArrowFunctionBody, ParsedAssignment, ParsedBindingName,
    ParsedExpression, ParsedForOfStatement, ParsedFunctionBodyStatement, ParsedFunctionDeclaration,
    ParsedFunctionParameter, ParsedIfStatement, ParsedLogicalOperator, ParsedObjectBindingElement,
    ParsedObjectBindingPattern, ParsedReturnStatement, ParsedSwitchStatement, ParsedTryStatement,
    ParsedType, ParsedTypeParameter, ParsedUnaryOperator, ParsedVariableDeclaration,
    ParsedVariableKind, ParsedWhileStatement,
};
use typescript_rust_types::{
    FunctionType, Type, TypeCopyReason, is_assignable_to, union_type, with_type_copy_reason,
};

use super::assign::check_assignment_with_symbols;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use super::expr::evaluate_expression;
use super::ops;
use super::var::{
    VariableCheckOptions, check_variable_declaration_against_symbols,
    widen_implicit_variable_initializer_type,
};
use crate::arena::alloc_function_type;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{
    AssignmentState, FlowCheck, FunctionFlowState, analyze_function_body_flow,
    apply_variable_declaration_state, check_assignment_target_flow, check_expression_flow,
    check_obvious_truthiness_condition, collect_function_flow_facts,
    collect_future_block_scoped_declarations, mark_assignment_state, merge_branch_deltas,
};
use crate::infer::{
    InferredExpression, TypeParameterSubstitution, map_parsed_type,
    map_parsed_type_with_substitution, report_duplicate_type_parameters,
};
use crate::program::{
    record_flow_function_count, record_flow_function_skipped_count, record_flow_statement_count,
    record_function_body_check, record_program_timing,
};
use crate::symbols::{
    FunctionSignatureInfo, ScopeStack, SymbolInfo, SymbolKind, SymbolTable,
    clone_symbol_info_handle,
};

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
        ParsedBindingName::Identifier { .. } => {
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone())
        }
        ParsedBindingName::ObjectPattern(_) | ParsedBindingName::Unsupported { .. } => Type::Any,
    }
}

fn insert_binding_name(binding_name: &ParsedBindingName, ty: Type, scopes: &mut ScopeStack) {
    match binding_name {
        ParsedBindingName::Identifier { name, .. } => {
            scopes.insert_current(
                name.as_str(),
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
        insert_object_binding_element_binding(
            element,
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || parameter_type.clone()),
            scopes,
        );
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
                name.as_str(),
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
    let mut parameter_types = Vec::with_capacity(parameters.len());
    let mut parameter_symbols = None;
    let mut parameter_bindings: Vec<(String, Type)> = Vec::new();

    for (index, parameter) in parameters.iter().enumerate() {
        let inferred_parameter_type = if let Some(declared_type) = parameter.declared_type.clone() {
            map_parsed_type_with_substitution(declared_type, ctx, &type_parameter_substitution)
        } else if let Some(initializer) = parameter.initializer.as_ref() {
            let parameter_symbols = parameter_symbols.get_or_insert_with(|| {
                let mut symbols = ctx
                    .symbols
                    .clone_with_reason(TypeCopyReason::FunctionBodySetup);
                for (name, ty) in &parameter_bindings {
                    let _ = symbols.insert(
                        name.clone(),
                        SymbolInfo {
                            ty: with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                                ty.clone()
                            }),
                            kind: SymbolKind::Parameter,
                            function_signature: None,
                        },
                    );
                }
                symbols
            });
            let inferred_initializer = evaluate_expression(
                initializer,
                parameter.initializer_span,
                parameter_symbols,
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
            let parameter_binding_type =
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                    inferred_parameter_type.clone()
                });
            if let Some(parameter_symbols) = parameter_symbols.as_mut() {
                let _ = parameter_symbols.insert(
                    name.to_string(),
                    SymbolInfo {
                        ty: with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                            parameter_binding_type.clone()
                        }),
                        kind: SymbolKind::Parameter,
                        function_signature: None,
                    },
                );
            }
            parameter_bindings.push((name.to_string(), parameter_binding_type));
        }

        parameter_types.push(inferred_parameter_type);

        if ctx.options.no_implicit_any {
            let contextual_type = contextual_parameter_types.and_then(|types| types.get(index));
            emit_parameter_diagnostics(parameter, contextual_type, ctx);
        }
    }

    let function_return_type = return_type
        .map(|return_type| {
            with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                map_parsed_type_with_substitution(
                    return_type.clone(),
                    ctx,
                    &type_parameter_substitution,
                )
            })
        })
        .unwrap_or(Type::Unknown);

    alloc_function_type(
        parameter_types,
        function_return_type,
        false,
        required_parameter_count(parameters),
    )
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
        substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
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
    let flow_facts = collect_function_flow_facts(&body);

    let mut scopes = ScopeStack::from_root(merged_function_body_root_symbols(ctx));
    scopes.insert_current(
        name,
        SymbolInfo {
            ty: Type::Function(with_type_copy_reason(
                TypeCopyReason::FunctionBodySetup,
                || function_type.clone(),
            )),
            kind: SymbolKind::Function,
            function_signature,
        },
    );
    scopes.push_child();
    let mut flow_state = FunctionFlowState::new(
        flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
    );

    for (parameter, parameter_type) in parameters
        .into_iter()
        .zip(function_type.parameters().iter())
    {
        insert_parameter_bindings(&parameter, parameter_type, &mut scopes);
    }

    with_type_parameter_scope(type_parameters, ctx, |ctx| {
        check_function_body(
            body,
            Some(function_type.return_type()),
            &mut scopes,
            &mut flow_state,
            ctx,
        );
    });

    if has_explicit_return_type && should_check_missing_return(function_type.return_type()) {
        emit_missing_return_diagnostic(body_flow, ctx);
    }
}

fn merged_function_body_root_symbols(ctx: &CheckerContext) -> SymbolTable {
    let mut root = ctx
        .ambient_global_symbols
        .clone_with_reason(TypeCopyReason::FunctionBodySetup);
    for (name, symbol) in ctx.symbols.iter_handles() {
        root.insert_handle(name.clone(), clone_symbol_info_handle(symbol));
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

    let function_type = map_function_signature(
        &function.parameters,
        function.return_type.as_ref(),
        &function.type_parameters,
        None,
        ctx,
    );

    *symbols = std::mem::take(&mut ctx.symbols);

    let duplicate = register_function_signature(
        function.name.clone(),
        with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || function_type.clone()),
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
    let start = Instant::now();
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
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || function_type.clone()),
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
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.function_declaration_checking += start.elapsed()
    });
}

pub(crate) fn check_function_declaration_body(
    function: ParsedFunctionDeclaration,
    function_type: &FunctionType,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) {
    let start = Instant::now();
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
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.function_declaration_checking += start.elapsed()
    });
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
        is_async,
        body,
        span: arrow_span,
    } = arrow;
    let _ = is_async;

    let contextual_parameter_types = expected_type.map(|expected_type| expected_type.parameters());
    with_type_parameter_scope(&type_parameters, ctx, |ctx| {
        let function_type = map_function_signature(
            &parameters,
            return_type.as_ref(),
            &type_parameters,
            contextual_parameter_types,
            ctx,
        );
        let source_type_name = function_type.name();
        let has_explicit_return_type = return_type.is_some();
        let mut parameter_types = function_type.parameters().to_vec();
        let mut return_type = with_type_copy_reason(TypeCopyReason::ExpectedType, || {
            function_type.return_type().clone()
        });

        if let Some(expected_type) = expected_type {
            for (index, parameter_type) in expected_type.parameters().iter().cloned().enumerate() {
                if index < parameter_types.len() && parameters[index].declared_type.is_none() {
                    parameter_types[index] = parameter_type;
                }
            }

            if !has_explicit_return_type {
                return_type = with_type_copy_reason(TypeCopyReason::ExpectedType, || {
                    expected_type.return_type().clone()
                });
            }

            if has_contextual_unknown_object_binding_pattern(
                &parameters,
                contextual_parameter_types,
            ) {
                let target_type_name = expected_type.name();
                let diagnostic =
                    Diagnostic::ts2345(&source_type_name, &target_type_name, ctx.file_name.clone());
                let diagnostic = match arrow_span {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };
                ctx.push(diagnostic);
            }
        }

        let mut scopes =
            ScopeStack::from_root(symbols.clone_with_reason(TypeCopyReason::FunctionBodySetup));
        scopes.push_child();
        for (index, parameter) in parameters.iter().enumerate() {
            let parameter_type = parameter_types.get(index).unwrap_or(&Type::Any);
            insert_parameter_bindings(parameter, parameter_type, &mut scopes);
        }

        let visible_symbols = visible_symbols(&scopes);
        match body {
            ParsedArrowFunctionBody::Expression(expression) => {
                let return_type_for_body = match &return_type {
                    Type::Any | Type::Unknown | Type::Void => None,
                    ty => Some(ty),
                };
                let inferred_body = match return_type_for_body {
                    None => evaluate_expression(&expression, None, &visible_symbols, ctx),
                    Some(return_type_for_body) => evaluate_expression_with_expected_type(
                        &expression,
                        None,
                        Some(return_type_for_body),
                        ExpectedTypeDiagnostic::TypeNotAssignable,
                        &visible_symbols,
                        ctx,
                    ),
                };

                if !has_explicit_return_type {
                    if let InferredExpression::Known(body_type) = inferred_body {
                        if body_type != Type::Unknown {
                            return_type = body_type;
                        }
                    }
                }
            }
            ParsedArrowFunctionBody::Block(statements) => {
                let flow_facts = collect_function_flow_facts(&statements);
                let mut flow_state = FunctionFlowState::new(
                    flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
                );
                let return_type_for_body = match &return_type {
                    Type::Any | Type::Unknown | Type::Void => None,
                    ty => Some(ty),
                };
                check_function_body(
                    statements,
                    return_type_for_body,
                    &mut scopes,
                    &mut flow_state,
                    ctx,
                );
            }
        }

        if expected_type
            .is_some_and(|expected_type| matches!(expected_type.return_type(), Type::Void))
            && !has_explicit_return_type
        {
            return_type = Type::Void;
        }

        alloc_function_type(
            parameter_types,
            return_type,
            function_type.is_variadic(),
            function_type.required_parameter_count(),
        )
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
            function.parameters().iter().any(type_contains_unknown)
                || type_contains_unknown(function.return_type())
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
        Type::Union(union) => union.types().iter().any(type_contains_unknown),
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
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    record_function_body_check();
    record_flow_function_count();

    let mut pushed_scope = false;
    if flow_state.is_enabled() {
        let future_block_scoped_declarations = collect_future_block_scoped_declarations(&body);
        if !future_block_scoped_declarations.is_empty() {
            flow_state.push_scope(future_block_scoped_declarations);
            pushed_scope = true;
        }
    } else {
        record_flow_function_skipped_count();
    }

    for (statement_index, statement) in body.into_iter().enumerate() {
        check_function_body_statement(
            statement,
            statement_index,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
    }

    if pushed_scope {
        flow_state.pop_scope();
    }
}

fn check_function_body_statement(
    statement: ParsedFunctionBodyStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    record_flow_statement_count();
    match statement {
        ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
            let start = Instant::now();
            check_function_variable_declaration(variable, statement_index, scopes, flow_state, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.variable_declaration_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Block(block_body) => {
            check_function_block(block_body, return_type, scopes, flow_state, ctx);
        }
        ParsedFunctionBodyStatement::Return(return_statement) => {
            let start = Instant::now();
            let visible_symbols = visible_symbols(scopes);
            check_function_return_statement(
                return_statement,
                statement_index,
                return_type,
                flow_state,
                &visible_symbols,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.return_statement_checking += start.elapsed()
            });
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
            let start = Instant::now();
            check_function_assignment(assignment, statement_index, scopes, flow_state, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.assignability_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Expression(expression) => {
            let start = Instant::now();
            check_function_expression_statement(
                expression,
                statement_index,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.expression_statement_checking += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::If(if_statement) => {
            let start = Instant::now();
            check_function_if_statement(
                if_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            let start = Instant::now();
            check_function_while_statement(
                while_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::ForOf(for_of_statement) => {
            let start = Instant::now();
            check_function_for_of_statement(
                for_of_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            let start = Instant::now();
            check_function_switch_statement(
                switch_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            let start = Instant::now();
            check_function_try_statement(
                try_statement,
                statement_index,
                return_type,
                scopes,
                flow_state,
                ctx,
            );
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.flow_narrowing += start.elapsed()
            });
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
        if flow_state.tracked_local_count() == 0 {
            return false;
        }

        flow_state.begin_branch_capture();
        if matches!(
            variable_kind,
            ParsedVariableKind::Let | ParsedVariableKind::Const
        ) {
            flow_state.declare_current(local_name.as_str(), AssignmentState::DeclaredUnassigned);
        }

        let blocked = check_expression_flow(
            initializer,
            variable.initializer_span,
            flow_state,
            statement_index,
            ctx,
        )
        .is_blocked();
        let _ = flow_state.finish_branch_capture();
        blocked
    });

    let visible_symbols = visible_symbols(scopes);

    if let Some(symbol) = check_variable_declaration_against_symbols(
        variable,
        visible_symbols,
        ctx,
        VariableCheckOptions {
            report_duplicate_let_const: false,
            check_initializer: !initializer_flow_blocked,
        },
    ) {
        scopes.insert_current_handle(local_name.as_str(), symbol);
        apply_variable_declaration_state(variable_kind, local_name, has_initializer, flow_state);
    }
}

fn check_function_block(
    block_body: Vec<ParsedFunctionBodyStatement>,
    return_type: Option<&Type>,
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
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    check_obvious_truthiness_condition(&if_statement.condition, if_statement.condition_span, ctx);

    let then_guarantees_value_return =
        analyze_function_body_flow(&if_statement.then_body).guarantees_value_return;
    let has_else_body = !if_statement.else_body.is_empty();

    let flow_active = flow_state.tracked_local_count() > 0;
    let condition_blocked = if flow_active {
        check_expression_flow(
            &if_statement.condition,
            if_statement.condition_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_condition_expression_with_truthy_guards(
            &if_statement.condition,
            if_statement.condition_span,
            &visible_symbols,
            ctx,
        );
    }

    if flow_active {
        let mut branch_deltas = Vec::new();
        scopes.push_child();
        flow_state.begin_branch_capture();
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let mut then_delta = flow_state.finish_branch_capture();
        then_delta.continues = !then_guarantees_value_return;
        scopes.pop_child();
        branch_deltas.push(then_delta);

        if has_else_body {
            let else_guarantees_value_return =
                analyze_function_body_flow(&if_statement.else_body).guarantees_value_return;
            scopes.push_child();
            flow_state.begin_branch_capture();
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            let mut else_delta = flow_state.finish_branch_capture();
            else_delta.continues = !else_guarantees_value_return;
            scopes.pop_child();
            branch_deltas.push(else_delta);
        }

        if !has_else_body && then_guarantees_value_return {
            narrow_truthy_guarded_identifiers(&if_statement.condition, scopes);
        }

        merge_branch_deltas(flow_state, &branch_deltas, !has_else_body);
    } else {
        scopes.push_child();
        check_function_body(
            if_statement.then_body,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();

        if has_else_body {
            scopes.push_child();
            check_function_body(if_statement.else_body, return_type, scopes, flow_state, ctx);
            scopes.pop_child();
        }

        if !has_else_body && then_guarantees_value_return {
            narrow_truthy_guarded_identifiers(&if_statement.condition, scopes);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TruthyGuardTarget {
    Identifier(String),
    Property { base: String, property: String },
}

fn narrow_truthy_guarded_identifiers(condition: &ParsedExpression, scopes: &mut ScopeStack) {
    let mut targets = Vec::new();
    collect_truthy_guarded_identifiers(condition, &mut targets);

    for target in targets {
        let base_name = match &target {
            TruthyGuardTarget::Identifier(name) => name,
            TruthyGuardTarget::Property { base, .. } => base,
        };

        let Some(symbol) = scopes.resolve(base_name) else {
            continue;
        };

        let narrowed = match &target {
            TruthyGuardTarget::Identifier(_) => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    typescript_rust_types::remove_undefined(&symbol.ty)
                })
            }
            TruthyGuardTarget::Property { property, .. } => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    narrow_truthy_guarded_property(&symbol.ty, property)
                })
            }
        };

        if narrowed == symbol.ty {
            continue;
        }

        let _ = scopes.update_visible(
            base_name,
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
    targets: &mut Vec<TruthyGuardTarget>,
) {
    match condition {
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } => {
            collect_truthy_guarded_identifiers(left, targets);
            collect_truthy_guarded_identifiers(right, targets);
        }
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => {
            if let Some(target) = truthy_guard_target(operand) {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }
        _ => {}
    }
}

fn truthy_guard_target(expression: &ParsedExpression) -> Option<TruthyGuardTarget> {
    match expression {
        ParsedExpression::Identifier { name, .. } => {
            Some(TruthyGuardTarget::Identifier(name.clone()))
        }
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            ..
        } => truthy_guard_base_identifier(object).map(|base| TruthyGuardTarget::Property {
            base,
            property: property_name.clone(),
        }),
        ParsedExpression::NonNullAssertion { expression, .. } => truthy_guard_target(expression),
        _ => None,
    }
}

fn truthy_guard_base_identifier(expression: &ParsedExpression) -> Option<String> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some(name.clone()),
        ParsedExpression::NonNullAssertion { expression, .. } => {
            truthy_guard_base_identifier(expression)
        }
        _ => None,
    }
}

fn narrow_truthy_guarded_property(ty: &Type, property: &str) -> Type {
    let narrowed_base = typescript_rust_types::remove_undefined(ty);

    match narrowed_base {
        Type::Object(mut object_type) => {
            if let Some(existing) = object_type.properties.get(property).cloned() {
                let properties = Arc::make_mut(&mut object_type.properties);
                properties.insert(
                    property.to_string(),
                    typescript_rust_types::ObjectProperty {
                        ty: typescript_rust_types::remove_undefined(&existing.ty),
                        optional: false,
                    },
                );
            }

            Type::Object(object_type)
        }
        Type::Union(union) => union_type(
            union
                .types()
                .iter()
                .map(|member| narrow_truthy_guarded_property(member, property))
                .collect(),
        ),
        _ => narrowed_base,
    }
}

fn check_function_while_statement(
    while_statement: ParsedWhileStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    check_obvious_truthiness_condition(
        &while_statement.condition,
        while_statement.condition_span,
        ctx,
    );

    let flow_active = flow_state.tracked_local_count() > 0;
    let condition_blocked = if flow_active {
        check_expression_flow(
            &while_statement.condition,
            while_statement.condition_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_condition_expression_with_truthy_guards(
            &while_statement.condition,
            while_statement.condition_span,
            &visible_symbols,
            ctx,
        );
    }

    scopes.push_child();
    if flow_active {
        flow_state.begin_branch_capture();
        check_function_body(while_statement.body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(while_statement.body, return_type, scopes, flow_state, ctx);
    }
    scopes.pop_child();
}

fn check_function_for_of_statement(
    for_of_statement: ParsedForOfStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let flow_active = flow_state.tracked_local_count() > 0;
    let iterable_blocked = if flow_active {
        check_expression_flow(
            &for_of_statement.iterable,
            for_of_statement.iterable_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    let mut element_type = Type::Unknown;
    if !iterable_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        if let InferredExpression::Known(iterable_type) = evaluate_expression(
            &for_of_statement.iterable,
            for_of_statement.iterable_span,
            &visible_symbols,
            ctx,
        ) {
            element_type = for_of_element_type(&iterable_type);
        }
    }

    scopes.push_child();
    insert_binding_name(&for_of_statement.binding_name, element_type, scopes);
    if flow_active {
        flow_state.begin_branch_capture();
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
        let _ = flow_state.finish_branch_capture();
    } else {
        check_function_body(for_of_statement.body, return_type, scopes, flow_state, ctx);
    }
    scopes.pop_child();
}

fn for_of_element_type(iterable_type: &Type) -> Type {
    match iterable_type {
        Type::Any => Type::Any,
        Type::Array(element) => with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
            element.as_ref().clone()
        }),
        Type::Tuple(elements) => {
            if elements.is_empty() {
                Type::Unknown
            } else {
                with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || {
                    union_type(elements.clone())
                })
            }
        }
        Type::String | Type::StringLiteral(_) => Type::String,
        Type::Union(union) => {
            let element_types = union
                .types()
                .iter()
                .filter(|ty| **ty != Type::Undefined)
                .map(for_of_element_type)
                .collect::<Vec<_>>();

            if element_types.is_empty() {
                Type::Unknown
            } else {
                union_type(element_types)
            }
        }
        _ => Type::Unknown,
    }
}

fn check_function_switch_statement(
    switch_statement: ParsedSwitchStatement,
    statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let flow_active = flow_state.tracked_local_count() > 0;
    let condition_blocked = if flow_active {
        check_expression_flow(
            &switch_statement.discriminant,
            switch_statement.discriminant_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if !condition_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let _ = evaluate_expression(
            &switch_statement.discriminant,
            switch_statement.discriminant_span,
            &visible_symbols,
            ctx,
        );
    }

    if flow_active {
        let mut branch_deltas = Vec::new();

        for switch_case in switch_statement.cases {
            let case_guarantees_value_return =
                analyze_function_body_flow(&switch_case.consequent).guarantees_value_return;
            if let Some(test) = switch_case.test.as_ref() {
                let _ = check_expression_flow(
                    test,
                    switch_case.test_span,
                    flow_state,
                    statement_index,
                    ctx,
                );
            }

            scopes.push_child();
            flow_state.begin_branch_capture();
            check_function_body(
                switch_case.consequent,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            let mut case_delta = flow_state.finish_branch_capture();
            case_delta.continues = !case_guarantees_value_return;
            scopes.pop_child();
            branch_deltas.push(case_delta);
        }

        merge_branch_deltas(flow_state, &branch_deltas, false);
    } else {
        for switch_case in switch_statement.cases {
            scopes.push_child();
            check_function_body(
                switch_case.consequent,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            scopes.pop_child();
        }
    }
}

fn check_function_try_statement(
    try_statement: ParsedTryStatement,
    _statement_index: usize,
    return_type: Option<&Type>,
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let flow_active = flow_state.tracked_local_count() > 0;

    if flow_active {
        let mut branch_deltas = Vec::new();
        let try_guarantees_value_return =
            analyze_function_body_flow(&try_statement.block).guarantees_value_return;
        scopes.push_child();
        flow_state.begin_branch_capture();
        check_function_body(
            try_statement.block,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        let mut try_delta = flow_state.finish_branch_capture();
        try_delta.continues = !try_guarantees_value_return;
        scopes.pop_child();
        branch_deltas.push(try_delta);

        if let Some(handler_clause) = try_statement.handler {
            let catch_guarantees_value_return =
                analyze_function_body_flow(&handler_clause.body).guarantees_value_return;
            scopes.push_child();
            if let Some(binding_name) = handler_clause.binding_name.as_ref() {
                if let Some(declared_type) = handler_clause.declared_type.as_ref() {
                    if !matches!(declared_type, ParsedType::Any | ParsedType::Unknown) {
                        let mut diagnostic = Diagnostic::new(
                            DiagnosticCode::TypeScript(1196),
                            "Catch clause variable type annotation must be 'any' or 'unknown' if specified.",
                            ctx.file_name.clone(),
                        );
                        if let ParsedBindingName::Identifier { span, .. } = binding_name {
                            if let Some(span) = span {
                                diagnostic = diagnostic.with_span(convert_span(*span));
                            }
                        }
                        ctx.push(diagnostic);
                    }
                }

                let catch_type = handler_clause
                    .declared_type
                    .clone()
                    .map(|ty| map_parsed_type(ty, ctx))
                    .unwrap_or(Type::Unknown);
                insert_binding_name(binding_name, catch_type, scopes);
            }
            flow_state.begin_branch_capture();
            check_function_body(
                handler_clause.body,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            let mut catch_delta = flow_state.finish_branch_capture();
            catch_delta.continues = !catch_guarantees_value_return;
            scopes.pop_child();
            branch_deltas.push(catch_delta);
        }

        merge_branch_deltas(flow_state, &branch_deltas, false);
        scopes.push_child();
        check_function_body(
            try_statement.finalizer,
            return_type,
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();
    } else {
        scopes.push_child();
        check_function_body(
            try_statement.block,
            with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();

        if let Some(handler_clause) = try_statement.handler {
            scopes.push_child();
            if let Some(binding_name) = handler_clause.binding_name.as_ref() {
                if let Some(declared_type) = handler_clause.declared_type.as_ref() {
                    if !matches!(declared_type, ParsedType::Any | ParsedType::Unknown) {
                        let mut diagnostic = Diagnostic::new(
                            DiagnosticCode::TypeScript(1196),
                            "Catch clause variable type annotation must be 'any' or 'unknown' if specified.",
                            ctx.file_name.clone(),
                        );
                        if let ParsedBindingName::Identifier { span, .. } = binding_name {
                            if let Some(span) = span {
                                diagnostic = diagnostic.with_span(convert_span(*span));
                            }
                        }
                        ctx.push(diagnostic);
                    }
                }

                let catch_type = handler_clause
                    .declared_type
                    .clone()
                    .map(|ty| map_parsed_type(ty, ctx))
                    .unwrap_or(Type::Unknown);
                insert_binding_name(binding_name, catch_type, scopes);
            }
            check_function_body(
                handler_clause.body,
                with_type_copy_reason(TypeCopyReason::ReturnChecking, || return_type.clone()),
                scopes,
                flow_state,
                ctx,
            );
            scopes.pop_child();
        }

        scopes.push_child();
        check_function_body(
            try_statement.finalizer,
            return_type,
            scopes,
            flow_state,
            ctx,
        );
        scopes.pop_child();
    }
}

fn check_function_throw_statement(
    throw_statement: typescript_rust_syntax::ParsedThrowStatement,
    statement_index: usize,
    scopes: &ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    if flow_state.tracked_local_count() > 0 {
        let _ = check_expression_flow(
            &throw_statement.expression,
            throw_statement.expression_span,
            flow_state,
            statement_index,
            ctx,
        );
    }

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
    scopes: &mut ScopeStack,
    flow_state: &mut FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    let target_name = assignment.target_name.clone();

    let (target_blocked, value_blocked) = if flow_state.tracked_local_count() > 0 {
        (
            check_assignment_target_flow(
                &target_name,
                flow_state,
                statement_index,
                ctx,
                assignment.target_span,
            ),
            check_expression_flow(
                &assignment.value,
                assignment.value_span,
                flow_state,
                statement_index,
                ctx,
            ),
        )
    } else {
        (FlowCheck::Clear, FlowCheck::Clear)
    };

    if !target_blocked.is_blocked() && !value_blocked.is_blocked() {
        let visible_symbols = visible_symbols(scopes);
        let inferred_value = evaluate_expression(
            &assignment.value,
            assignment.value_span,
            &visible_symbols,
            ctx,
        );
        check_assignment_with_symbols(assignment, &visible_symbols, ctx);
        update_assigned_symbol_type(&target_name, inferred_value, scopes);
    }

    if !target_blocked.is_blocked() && flow_state.tracked_local_count() > 0 {
        mark_assignment_state(&target_name, flow_state);
    }
}

fn update_assigned_symbol_type(
    target_name: &str,
    inferred_value: InferredExpression,
    scopes: &mut ScopeStack,
) {
    let InferredExpression::Known(value_ty) = inferred_value else {
        return;
    };

    if value_ty == Type::Unknown {
        return;
    }

    let Some(symbol) = scopes.resolve(target_name) else {
        return;
    };

    let updated_ty = if symbol.ty == Type::Undefined {
        union_type(vec![
            Type::Undefined,
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || value_ty.clone()),
        ])
    } else if symbol.ty == value_ty || is_assignable_to(&value_ty, &symbol.ty) {
        with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
    } else if matches!(symbol.ty, Type::Any | Type::Unknown) {
        union_type(vec![
            with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone()),
            value_ty,
        ])
    } else {
        // Preserve the declared/inferred symbol type when an incompatible assignment
        // is already reported to avoid cascading return/usage diagnostics.
        with_type_copy_reason(TypeCopyReason::ScopeOrContext, || symbol.ty.clone())
    };

    if updated_ty == symbol.ty {
        return;
    }

    let _ = scopes.update_visible(
        target_name,
        SymbolInfo {
            ty: updated_ty,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
    );
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

    let flow_blocked = if flow_state.tracked_local_count() > 0 {
        check_expression_flow(&expression, None, flow_state, statement_index, ctx)
    } else {
        FlowCheck::Clear
    };

    if flow_blocked.is_blocked() {
        return;
    }

    let visible_symbols = visible_symbols(scopes);
    let _ = evaluate_expression(&expression, None, &visible_symbols, ctx);
}

fn evaluate_condition_expression_with_truthy_guards(
    expression: &ParsedExpression,
    fallback_span: Option<typescript_rust_syntax::TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match expression {
        ParsedExpression::Logical {
            left,
            left_span,
            operator: ParsedLogicalOperator::Or,
            right,
            right_span,
            ..
        } => {
            let left_result = evaluate_condition_expression_with_truthy_guards(
                left,
                left_span.or(fallback_span),
                symbols,
                ctx,
            );
            let narrowed_symbols = narrow_truthy_guarded_symbol_table(left, symbols);
            let right_result = evaluate_condition_expression_with_truthy_guards(
                right,
                right_span.or(fallback_span),
                &narrowed_symbols,
                ctx,
            );
            ops::evaluate_logical_expression(left_result, right_result)
        }
        _ => evaluate_expression(expression, fallback_span, symbols, ctx),
    }
}

fn narrow_truthy_guarded_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
) -> SymbolTable {
    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    let mut targets = Vec::new();
    collect_truthy_guarded_identifiers(condition, &mut targets);

    for target in targets {
        let base_name = match &target {
            TruthyGuardTarget::Identifier(name) => name,
            TruthyGuardTarget::Property { base, .. } => base,
        };

        let Some(symbol) = narrowed_symbols.get(base_name) else {
            continue;
        };

        let narrowed = match &target {
            TruthyGuardTarget::Identifier(_) => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    typescript_rust_types::remove_undefined(&symbol.ty)
                })
            }
            TruthyGuardTarget::Property { property, .. } => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    narrow_truthy_guarded_property(&symbol.ty, property)
                })
            }
        };

        if narrowed == symbol.ty {
            continue;
        }

        narrowed_symbols.insert(
            base_name.clone(),
            SymbolInfo {
                ty: narrowed,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }

    narrowed_symbols
}

fn visible_symbols(scopes: &ScopeStack) -> &SymbolTable {
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
    return_type: Option<&Type>,
    flow_state: &mut FunctionFlowState,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(expression) = return_statement.expression.as_ref() else {
        return;
    };

    let flow_blocked = if flow_state.tracked_local_count() > 0 {
        check_expression_flow(
            expression,
            return_statement.expression_span,
            flow_state,
            statement_index,
            ctx,
        )
    } else {
        FlowCheck::Clear
    };

    if flow_blocked.is_blocked() {
        return;
    }

    let Some(return_type) = return_type else {
        return;
    };

    let inferred_expression = evaluate_expression_with_expected_type(
        expression,
        return_statement.expression_span,
        Some(return_type),
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
