//! Built-in call shapes: Array.map/find, Promise.all.

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedCallArgument, TextSpan as SyntaxTextSpan};
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
    match object_type {
        Type::Object(object) => {
            object.contains_property("resolve") && object.contains_property("all")
        }
        _ => false,
    }
}

pub(crate) fn check_promise_all_call(
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

    match inferred {
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
        InferredExpression::Known(ty) => Some(Type::Array(Box::new(ty))),
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => Some(Type::Array(Box::new(Type::Any))),
    }
}
