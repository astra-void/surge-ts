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
mod body_statements;
mod narrowing;
mod signature;

pub(crate) use body::*;
pub(crate) use body_statements::*;
pub(crate) use narrowing::*;
pub(crate) use signature::*;
pub(crate) fn collect_function_declaration_signature(
    function: &ParsedFunctionDeclaration,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
    allow_lazy_dependency_signature: bool,
) -> FunctionType {
    let temp_symbols = std::mem::take(symbols);
    ctx.set_symbols(temp_symbols);

    // Establish the function's own type-parameter scope (with constraints) while
    // mapping the signature, so a constrained parameter such as `K extends keyof T`
    // is visible when its body resolves a `T[K]` indexed access (otherwise a false
    // TS2536, e.g. the lib `addEventListener<K extends keyof WindowEventMap>(…:
    // WindowEventMap[K])`).
    //
    // Scoped to *generic* `declare` functions only. For a `declare` function this
    // collected signature is the authoritative resolution (no body is checked
    // afterwards), so it must resolve its constrained indexed accesses here. A
    // non-`declare` function is re-checked authoritatively by
    // `check_function_declaration` under its own scope; resolving its (often
    // cross-module) signature concretely *here* instead changed how generic
    // instantiations were collected and surfaced assignability false positives, so
    // its pre-pass signature is left as-is. The empty-scope case is also skipped: a
    // pushed empty scope makes `type_parameter_scopes` non-empty and flips the
    // `concrete_instantiation` short-circuit.
    crate::program::record_program_counter(|c| c.function_signatures_indexed_count += 1);
    let map_signature = |ctx: &mut CheckerContext| {
        static LAZY_DEPENDENCY_SIGNATURES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let lazy_dependency_signatures = *LAZY_DEPENDENCY_SIGNATURES
            .get_or_init(|| std::env::var_os("SURGE_EAGER_DEPENDENCY_SIGNATURES").is_none());
        if ctx.current_file_kind == crate::context::FileKind::DependencyDeclaration
            && allow_lazy_dependency_signature
            && lazy_dependency_signatures
        {
            map_lazy_dependency_function_signature(function, ctx)
        } else {
            map_function_signature(
                &function.parameters,
                function.return_type.as_ref(),
                &function.type_parameters,
                None,
                ctx,
            )
        }
    };
    let function_type = if function.is_declare && !function.type_parameters.is_empty() {
        with_type_parameter_scope(&function.type_parameters, ctx, map_signature)
    } else {
        map_signature(ctx)
    };

    *symbols = std::mem::take(&mut ctx.symbols);

    let duplicate = register_function_signature(
        function.name.clone(),
        with_type_copy_reason(TypeCopyReason::FunctionBodySetup, || function_type.clone()),
        Some(function_signature_info(
            &function.type_parameters,
            &function.parameters,
            function.return_type.as_ref(),
            &ctx.file_name,
        )),
        symbols,
        false,
        function.has_body,
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
        let signature_info = function_signature_info(
            &type_parameters,
            &parameters,
            return_type.as_ref(),
            &ctx.file_name,
        );
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
                has_body,
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

        // A bodyless `function` declaration is an ambient declaration or an
        // overload signature: there is no body to check, and the implementation
        // (or none, for ambient) carries the real body. Running body checks here
        // would falsely flag the signature (e.g. TS2355 for a non-void return type
        // with no `return`), which tsc never does for an overload signature.
        if is_declare || !has_body {
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

    // A bodyless declaration is ambient or an overload signature; checking the
    // absent body would report TS2355 on a non-void return type that the
    // implementation below actually satisfies. Mirrors the guard in
    // `check_function_declaration`.
    if is_declare || !has_body {
        return;
    }

    let signature_info = function_signature_info(
        type_parameters,
        &parameters,
        return_type.as_ref(),
        &ctx.file_name,
    );
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

/// The contextual type a written parameter takes at each position of
/// `expected_type`. A rest parameter is not one positional slot: a tuple-typed
/// one (`(...args: [value: number, message?: string]) => void`, the shape TS
/// variadic-tuple reconstruction produces) declares those elements positionally,
/// and an array-typed one supplies its element type at every trailing position.
/// Reading `parameters()` by index instead leaves every position past the rest
/// slot uncontextualized, which is a false TS7006/TS7031 on the callback.
/// The tuple expansion mirrors `expanded_signature` in the assignability
/// relation, so a callback typed here still compares against its slot.
fn contextual_parameter_types(expected_type: &FunctionType, parameter_count: usize) -> Vec<Type> {
    let parameters = expected_type.parameters();
    if !expected_type.is_variadic() {
        return parameters.to_vec();
    }

    let Some((rest, leading)) = parameters.split_last() else {
        return Vec::new();
    };
    let peeled;
    let rest = match rest {
        Type::Reference(reference) => {
            peeled = reference.resolve().peeled();
            &peeled
        }
        other => other,
    };

    let mut expanded = leading.to_vec();
    match rest {
        Type::Tuple(elements) => expanded.extend(elements.iter().cloned()),
        // The resolver already unwraps an array rest annotation to its element
        // type, but a signature mapped from source keeps the array; accept both.
        other => {
            let element = match other {
                Type::Array(element) => element.as_ref(),
                other => other,
            };
            let fill_to = parameter_count.max(leading.len() + 1);
            while expanded.len() < fill_to {
                expanded.push(element.clone());
            }
        }
    }
    expanded
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

    let expanded_contextual_parameter_types = expected_type
        .map(|expected_type| contextual_parameter_types(expected_type, parameters.len()));
    let contextual_parameter_types = expanded_contextual_parameter_types.as_deref();
    with_type_parameter_scope(&type_parameters, ctx, |ctx| {
        // Resolve the arrow's annotations against the value symbols visible at
        // the arrow site, mirroring `check_variable_declaration_against_symbols`:
        // `(x: typeof localConst) => …` must see the enclosing function body's
        // locals, which live in the scope stack and never reach `ctx.symbols`.
        let saved_symbols = std::mem::replace(&mut ctx.symbols, symbols.clone());
        let function_type = map_function_signature(
            &parameters,
            return_type.as_ref(),
            &type_parameters,
            contextual_parameter_types,
            ctx,
        );
        ctx.symbols = saved_symbols;
        let source_type_name = function_type.name();
        let has_explicit_return_type = return_type.is_some();
        let mut parameter_types = function_type.parameters().to_vec();
        let mut return_type = with_type_copy_reason(TypeCopyReason::ExpectedType, || {
            function_type.return_type().clone()
        });

        if let Some(expected_type) = expected_type {
            for (index, parameter_type) in contextual_parameter_types
                .unwrap_or_default()
                .iter()
                .cloned()
                .enumerate()
            {
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
                    Type::Any | Type::Unknown | Type::GenuineUnknown | Type::Void => None,
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
                        if !body_type.is_unknown() {
                            return_type = body_type;
                        }
                    }
                }
            }
            ParsedArrowFunctionBody::Block(statements) => {
                emit_unused_locals(&statements, &body_reads, ctx);
                let flow_facts = collect_function_flow_facts(&statements);
                let mut flow_state = FunctionFlowState::new(
                    flow_facts.has_let_or_const || flow_facts.has_future_block_scoped_declarations,
                );
                let body_flow = analyze_function_body_flow(&statements);
                let return_type_for_body = match &return_type {
                    Type::Any | Type::Unknown | Type::GenuineUnknown | Type::Void => None,
                    ty => Some(ty),
                };
                // A contextual return type this arrow did not annotate is not a
                // per-return assignability target for tsc — see
                // `ContextualReturnFrame`.
                let contextual_return_frame = !has_explicit_return_type && expected_type.is_some();
                if contextual_return_frame {
                    ctx.open_contextual_return_frame();
                }
                check_function_body(
                    statements,
                    return_type_for_body,
                    &mut scopes,
                    &mut flow_state,
                    ctx,
                );
                if contextual_return_frame {
                    ctx.close_contextual_return_frame();
                }

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
