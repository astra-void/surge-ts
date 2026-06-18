//! Built-in call shapes: Array.map/find, Promise.all.

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedCallArgument, TextSpan as SyntaxTextSpan};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::arena::alloc_function_type;
use crate::checks::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
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
        1,
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
        InferredExpression::Known(Type::Unknown) => None,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => None,
        InferredExpression::Known(other) => Some(Type::Array(Box::new(other))),
    }
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

    let callback_type = Type::Function(alloc_function_type(
        array_iteration_callback_parameters(element_type),
        Type::Boolean,
        false,
        1,
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
        InferredExpression::Known(Type::Unknown) => None,
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
