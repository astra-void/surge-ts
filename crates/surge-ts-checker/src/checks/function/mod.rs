use std::time::Instant;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedArrowFunction, ParsedArrowFunctionBody, ParsedFunctionDeclaration, ParsedTypeParameter,
};
use surge_ts_types::{FunctionType, Type, TypeCopyReason, with_type_copy_reason};

use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use super::expr::evaluate_expression;
use crate::arena::alloc_function_type;
use crate::context::CheckerContext;
use crate::context::convert_span;
use crate::flow::{FunctionFlowState, analyze_function_body_flow, collect_function_flow_facts};
use crate::infer::InferredExpression;
use crate::program::record_program_timing;
use crate::symbols::{ScopeStack, SymbolTable};

mod body;
mod narrowing;
mod signature;

pub(crate) use body::*;
pub(crate) use narrowing::*;
pub(crate) use signature::*;
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

        if let Some(first_span) = symbols.take_declaration_span(&function.name) {
            ctx.push(Diagnostic::ts2393(ctx.file_name.clone()).with_span(convert_span(first_span)));
        }
    } else if let Some(span) = function.name_span {
        symbols.record_declaration_span(&function.name, span);
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
        return_type_span,
        body,
        has_body,
        body_reads,
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

            if let Some(first_span) = ctx.symbols.take_declaration_span(&name) {
                ctx.push(
                    Diagnostic::ts2393(ctx.file_name.clone()).with_span(convert_span(first_span)),
                );
            }
        } else if let Some(span) = name_span {
            ctx.symbols.record_declaration_span(&name, span);
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
            return_type_span.or(name_span),
            has_body.then(|| body_reads.as_slice()),
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
        name_span,
        parameters,
        return_type,
        return_type_span,
        body,
        has_body,
        body_reads,
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
        return_type_span.or(name_span),
        has_body.then(|| body_reads.as_slice()),
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
        body_reads,
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

        if should_track_unused_parameters(ctx) {
            emit_unused_parameters(&parameters, &body_reads, ctx);
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
                let body_flow = analyze_function_body_flow(&statements);
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

                let contextually_void = expected_type
                    .is_some_and(|expected_type| matches!(expected_type.return_type(), Type::Void));
                if !has_explicit_return_type
                    && !contextually_void
                    && ctx.options.no_implicit_returns
                    && body_flow.contains_return_with_value
                    && !body_flow.guarantees_exit
                {
                    emit_implicit_return_diagnostic(arrow_span, ctx);
                }
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
