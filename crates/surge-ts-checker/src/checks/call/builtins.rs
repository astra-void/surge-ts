//! Built-in call shapes: Array.map/find, Promise.all.

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedCallArgument, ParsedExpression, TextSpan as SyntaxTextSpan};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::checks::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::metrics::alloc_function_type;
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::SymbolTable;

/// The `(element, index, array)` parameter list for an array iteration
/// callback. Supplying all three (rather than just the element) lets a
/// `(v, i) => …` callback contextually type its index as `number` instead of
/// leaving it implicitly `any` (`TS7006`).
fn array_iteration_callback_parameters(element_type: &Type) -> Vec<Type> {
    let element = with_type_copy_reason(TypeCopyReason::PropertyCallResolution, || {
        element_type.clone()
    });
    let array = with_type_copy_reason(TypeCopyReason::PropertyCallResolution, || {
        Type::Array(Box::new(element_type.clone()))
    });
    vec![element, Type::Number, array]
}

pub(crate) fn check_array_map_call(
    element_type: &Type,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    if arguments.is_empty() {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2554(1, 0, ctx.file_name.clone()),
            call_span.or(property_span),
        ));
        return None;
    }

    let callback_type = Type::Function(alloc_function_type(
        array_iteration_callback_parameters(element_type),
        Type::Any,
        false,
        // `(value, index, array)` are all required in the lib's signature.
        3,
    ));

    let inferred_callback = evaluate_expression_with_expected_type(
        &arguments[0].expression,
        arguments[0].span,
        Some(&callback_type),
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );

    match inferred_callback {
        InferredExpression::Known(Type::Function(function_type)) => Some(Type::Array(Box::new(
            with_type_copy_reason(TypeCopyReason::PropertyCallResolution, || {
                function_type.return_type().clone()
            }),
        ))),
        InferredExpression::Known(Type::Any) => Some(Type::Array(Box::new(Type::Any))),
        InferredExpression::Known(Type::Unknown)
        | InferredExpression::Known(Type::GenuineUnknown) => None,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => None,
        InferredExpression::Known(other) => Some(Type::Array(Box::new(other))),
    }
}

/// `Array.prototype.reduce` / `reduceRight`, by the lib's three overloads:
/// with no initial value the accumulator is the element type; an initial value
/// that fits the element type keeps it (`reduce((a, x) => a + x, 0)`);
/// anything else is the generic overload, whose `U` is the explicit type
/// argument or the initial value's widened type. The callback's return is not
/// compared against `U` here — only its parameters are given their types.
pub(crate) fn check_array_reduce_call(
    element_type: &Type,
    type_arguments: &[surge_ts_syntax::ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    let (callback, initial) = match arguments {
        [callback, initial] => (callback, Some(initial)),
        [callback, ..] => (callback, None),
        [] => return Type::Any,
    };
    // A type argument or an initial value surge cannot type leaves the
    // accumulator `any`; the callback is still checked against that.
    let explicit = type_arguments
        .first()
        .map(|type_argument| crate::infer::map_parsed_type(type_argument.clone(), ctx))
        .map(|mapped| if mapped.is_unknown() { Type::Any } else { mapped });
    let accumulator = match initial {
        None => explicit.unwrap_or_else(|| element_type.clone()),
        Some(initial) => {
            let evaluated = evaluate_expression_with_expected_type(
                &initial.expression,
                initial.span,
                explicit.as_ref().filter(|explicit| !matches!(explicit, Type::Any)),
                ExpectedTypeDiagnostic::ArgumentNotAssignable,
                symbols,
                ctx,
            );
            match (explicit, evaluated) {
                (Some(explicit), _) => explicit,
                (None, InferredExpression::Known(initial_type)) if !initial_type.is_unknown() => {
                    let widened = crate::checks::var::widen_implicit_variable_initializer_type(
                        crate::symbols::SymbolKind::Let,
                        &initial.expression,
                        &initial_type,
                        false,
                    );
                    if surge_ts_types::is_assignable_to(&widened, element_type) {
                        element_type.clone()
                    } else {
                        widened
                    }
                }
                _ => Type::Any,
            }
        }
    };
    let callback_type = Type::Function(alloc_function_type(
        vec![
            accumulator.clone(),
            element_type.clone(),
            Type::Number,
            Type::Array(Box::new(element_type.clone())),
        ],
        Type::Any,
        false,
        4,
    ));
    let _ = evaluate_expression_with_expected_type(
        &callback.expression,
        callback.span,
        Some(&callback_type),
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );
    accumulator
}

pub(crate) fn check_array_find_call(
    element_type: &Type,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    if arguments.is_empty() {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2554(1, 0, ctx.file_name.clone()),
            call_span.or(property_span),
        ));
        return None;
    }

    // The lib's non-narrowing `find` overload types the predicate as returning
    // `unknown`, so any truthy value is accepted.
    let callback_type = Type::Function(alloc_function_type(
        array_iteration_callback_parameters(element_type),
        Type::Any,
        false,
        // `(value, index, array)` are all required in the lib's signature.
        3,
    ));

    let inferred_callback = evaluate_expression_with_expected_type(
        &arguments[0].expression,
        arguments[0].span,
        Some(&callback_type),
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );

    match inferred_callback {
        InferredExpression::Known(Type::Function(_)) => Some(surge_ts_types::union_type(vec![
            with_type_copy_reason(TypeCopyReason::PropertyCallResolution, || {
                element_type.clone()
            }),
            Type::Undefined,
        ])),
        InferredExpression::Known(Type::Any) => Some(Type::Any),
        InferredExpression::Known(Type::Unknown)
        | InferredExpression::Known(Type::GenuineUnknown) => None,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => None,
        InferredExpression::Known(other) => Some(other),
    }
}

pub(crate) fn is_promise_all_receiver(object_type: &Type) -> bool {
    // The lib's `PromiseConstructor` is usually still a lazy reference here.
    match object_type.peeled() {
        Type::Object(object) => {
            object.contains_property("resolve") && object.contains_property("all")
        }
        _ => false,
    }
}

/// `Promise.resolve(value)`: a promise of what `value` awaits to, and
/// `Promise<void>` with no argument.
pub(crate) fn check_promise_resolve_call(
    arguments: &[ParsedCallArgument],
    expected_return_type: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(argument) = arguments.first() else {
        return Some(super::promise_of(&Type::Void, ctx));
    };
    // `T` is inferred from the argument with the contextual return type as a
    // second source, so under `Promise<"data">` the literal `"data"` stays the
    // literal it is; with no such context it widens like any inferred `T`.
    let contextual = expected_return_type
        .map(super::awaited_type)
        .filter(|contextual| !contextual.is_unknown() && !matches!(contextual, Type::Any));
    let checkpoint = ctx.diagnostics().len();
    let evaluated = evaluate_expression_with_expected_type(
        &argument.expression,
        argument.span,
        contextual.as_ref(),
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );
    match evaluated {
        InferredExpression::Known(ty) if !ty.is_unknown() => {
            let fits_context = contextual
                .as_ref()
                .is_some_and(|contextual| surge_ts_types::is_assignable_to(&ty, contextual));
            let value = if fits_context {
                ty
            } else {
                crate::checks::var::widen_implicit_variable_initializer_type(
                    crate::symbols::SymbolKind::Let,
                    &argument.expression,
                    &ty,
                    false,
                )
            };
            Some(super::promise_of(&value, ctx))
        }
        // A value that does not fit the context is the enclosing relation's to
        // report, against the promise — not this argument's against `T`.
        _ => {
            if contextual.is_some() {
                ctx.truncate_diagnostics(checkpoint);
            }
            None
        }
    }
}

/// `Promise.race([a, b])`: a promise of what any one element awaits to — the
/// union the lib's `Awaited<T[number]>` names over an array-literal argument.
pub(crate) fn check_promise_race_call(
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let [argument] = arguments else {
        return None;
    };
    let ParsedExpression::ArrayLiteral { elements, .. } = &argument.expression else {
        return None;
    };
    if elements.is_empty() || elements.iter().any(|element| element.spread) {
        return None;
    }
    let _ = evaluate_expression_with_expected_type(
        &argument.expression,
        argument.span,
        None,
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );
    let reported = ctx.diagnostics().len();
    let mut awaited = Vec::with_capacity(elements.len());
    for element in elements {
        match crate::infer::infer_expression(&element.expression, symbols, ctx) {
            // An element's fresh literal widens as an inferred `T` does
            // (`[p, 1]` races to `string | number`); `as const` keeps it.
            InferredExpression::Known(ty) if !ty.is_unknown() => {
                awaited.push(super::awaited_type(
                    &crate::checks::var::widen_implicit_variable_initializer_type(
                        crate::symbols::SymbolKind::Let,
                        &element.expression,
                        &ty,
                        false,
                    ),
                ));
            }
            _ => {
                ctx.truncate_diagnostics(reported);
                return None;
            }
        }
    }
    ctx.truncate_diagnostics(reported);
    Some(super::promise_of(&surge_ts_types::union_type(awaited), ctx))
}

pub(crate) fn check_promise_all_call(
    arguments: &[ParsedCallArgument],
    call_span: Option<SyntaxTextSpan>,
    expected_return_type: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let result = promise_all_result(arguments, call_span, symbols, ctx)?;
    // The contextual return type also types the argument (an element written
    // `[a, b]` is a tuple under `Promise<[A, B][]>`), which this path does not
    // do; a result that misses the context is therefore not asserted.
    let contextual = expected_return_type
        .map(super::awaited_type)
        .filter(|contextual| !contextual.is_unknown() && !matches!(contextual, Type::Any));
    if let Some(contextual) = contextual
        && !surge_ts_types::is_assignable_to(&super::awaited_type(&result), &contextual)
    {
        return None;
    }
    Some(result)
}

fn promise_all_result(
    arguments: &[ParsedCallArgument],
    call_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    if arguments.is_empty() {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2554(1, 0, ctx.file_name.clone()),
            call_span,
        ));
        return None;
    }

    let inferred = evaluate_expression_with_expected_type(
        &arguments[0].expression,
        arguments[0].span,
        None,
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );

    // `Promise.all([a, b])` answers a tuple of what each element awaits to
    // (the lib's `{ -readonly [P in keyof T]: Awaited<T[P]> }` over a tuple
    // `T`), which is what `const [x, y] = await Promise.all([…])` destructures.
    if let ParsedExpression::ArrayLiteral { elements, .. } = &arguments[0].expression
        && !elements.is_empty()
        && !elements.iter().any(|element| element.spread)
        && matches!(inferred, InferredExpression::Known(_))
    {
        let reported = ctx.diagnostics().len();
        let awaited: Vec<Type> = elements
            .iter()
            .map(|element| match crate::infer::infer_expression(&element.expression, symbols, ctx) {
                InferredExpression::Known(ty) => super::awaited_type(&ty),
                _ => Type::Any,
            })
            .collect();
        ctx.truncate_diagnostics(reported);
        return Some(super::promise_of(&Type::Tuple(awaited), ctx));
    }

    let inferred = match inferred {
        InferredExpression::Known(ty) => InferredExpression::Known(match ty {
            Type::Array(element) => Type::Array(Box::new(super::awaited_type(&element))),
            other => other,
        }),
        other => other,
    };
    let all = match inferred {
        InferredExpression::Known(Type::Array(element_type)) => Some(Type::Array(Box::new(
            with_type_copy_reason(TypeCopyReason::PropertyCallResolution, || {
                (*element_type).clone()
            }),
        ))),
        InferredExpression::Known(Type::Tuple(elements)) => {
            Some(Type::Array(Box::new(if elements.is_empty() {
                Type::Any
            } else {
                surge_ts_types::union_type(elements)
            })))
        }
        InferredExpression::Known(Type::Any) => Some(Type::Array(Box::new(Type::Any))),
        // Any other iterable (a readonly tuple, a `Set`) has a shape this path
        // does not map; making one up would be asserted downstream.
        InferredExpression::Known(_) => None,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => Some(Type::Array(Box::new(Type::Any))),
    };
    all.map(|all| super::promise_of(&all, ctx))
}
