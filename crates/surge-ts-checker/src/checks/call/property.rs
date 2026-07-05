//! Property-call and optional-property-call checking.

use super::*;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedCallArgument, ParsedExpression, ParsedType, TextSpan as SyntaxTextSpan,
};
use surge_ts_types::{FunctionType, Type, union_type};

use crate::checks::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::modules::{PROMISE_LIKE_VALUE_PROPERTY, promise_like_type};
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::SymbolTable;

pub(crate) fn check_property_call_like(
    object: &ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    property_name: &str,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let object_ty =
        match crate::checks::expr::evaluate_expression(object, object_span, symbols, ctx) {
            crate::infer::InferredExpression::Known(ty) => ty,
            _ => return None,
        };

    let object_type_name = object_ty.name();

    if property_name == "all" && is_promise_all_receiver(&object_ty) {
        return check_promise_all_call(arguments, call_span.or(property_span), symbols, ctx);
    }
    if property_name == "then"
        && let Some(value_type) = promise_like_value_type(&object_ty)
    {
        return check_promise_then_call(value_type, arguments, symbols, ctx);
    }

    match object_ty {
        Type::Any => Some(Type::Any),
        Type::Unknown | Type::GenuineUnknown => None,
        Type::Array(element_type) if property_name == "map" => check_array_map_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        ),
        Type::Array(element_type) if property_name == "find" => check_array_find_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        ),
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if *ty == Type::Undefined {
                    result_types.push(Type::Undefined);
                    continue;
                }

                if property_name == "map"
                    && let Type::Array(element_type) = ty
                {
                    let mapped = check_array_map_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(mapped);
                    continue;
                }

                if property_name == "find"
                    && let Type::Array(element_type) = ty
                {
                    let found = check_array_find_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(found);
                    continue;
                }

                if property_name == "find"
                    && let Type::Array(element_type) = ty
                {
                    let found = check_array_find_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(found);
                    continue;
                }

                let Some(property_type) = ty.get_property_access_type(property_name) else {
                    if no_lib_array_member(ty, ctx) {
                        result_types.push(Type::Any);
                        continue;
                    }
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone()),
                        crate::spans::choose_span(property_span, object_span),
                    ));
                    return None;
                };

                match callable_property_signature(property_type) {
                    Type::Function(function_type) => {
                        let return_type = check_function_type_call(
                            &function_type,
                            property_span,
                            call_span,
                            type_arguments,
                            arguments,
                            symbols,
                            ctx,
                        )?;
                        result_types.push(return_type);
                    }
                    Type::Any => result_types.push(Type::Any),
                    Type::Unknown | Type::GenuineUnknown => return None,
                    _ => {
                        ctx.push(diagnostic_with_syntax_span(
                            Diagnostic::ts2349(ctx.file_name.clone()),
                            crate::spans::choose_span(
                                call_span,
                                crate::spans::choose_span(property_span, object_span),
                            ),
                        ));
                        return None;
                    }
                }
            }

            Some(surge_ts_types::union_type(result_types))
        }
        _ => {
            if property_name == "map"
                && let Type::Array(element_type) = &object_ty
            {
                return check_array_map_call(
                    element_type.as_ref(),
                    property_span,
                    call_span,
                    arguments,
                    symbols,
                    ctx,
                );
            }

            let Some(property_type) = object_ty.get_property_access_type(property_name) else {
                if no_lib_array_member(&object_ty, ctx) {
                    return Some(Type::Any);
                }
                let diagnostic =
                    Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone());
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    crate::spans::choose_span(property_span, object_span),
                ));
                return None;
            };

            match callable_property_signature(property_type) {
                Type::Function(function_type) => check_function_type_call(
                    &function_type,
                    property_span,
                    call_span,
                    type_arguments,
                    arguments,
                    symbols,
                    ctx,
                ),
                Type::Any => Some(Type::Any),
                Type::Unknown | Type::GenuineUnknown => None,
                _ => {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2349(ctx.file_name.clone()),
                        crate::spans::choose_span(
                            call_span,
                            crate::spans::choose_span(property_span, object_span),
                        ),
                    ));
                    None
                }
            }
        }
    }
}

fn promise_like_value_type(ty: &Type) -> Option<Type> {
    let Type::Object(object_type) = ty.peeled() else {
        return None;
    };
    object_type
        .get_property_type(PROMISE_LIKE_VALUE_PROPERTY)
        .cloned()
}

fn check_promise_then_call(
    value_type: Type,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(callback) = arguments.first() else {
        return Some(promise_like_type(Type::Unknown));
    };

    let callback_type =
        Type::Function(FunctionType::new(vec![value_type], Type::Unknown, false, 1));
    let inferred_callback = evaluate_expression_with_expected_type(
        &callback.expression,
        callback.span,
        Some(&callback_type),
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );

    let next_value = match inferred_callback {
        InferredExpression::Known(Type::Function(function_type)) => {
            promise_like_awaited_type(function_type.return_type())
        }
        InferredExpression::Known(ty) => promise_like_awaited_type(&ty),
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => Type::Unknown,
    };

    Some(promise_like_type(next_value))
}

fn promise_like_awaited_type(ty: &Type) -> Type {
    if let Some(value_type) = promise_like_value_type(ty) {
        return value_type;
    }

    if let Type::Reference(reference) = ty {
        let base = reference
            .display
            .split('<')
            .next()
            .unwrap_or(&reference.display);
        if matches!(base, "Promise" | "PromiseLike")
            && let Some(value_type) = reference.arguments.first()
        {
            return value_type.clone();
        }
    }

    ty.clone()
}

pub(crate) fn check_optional_property_call(
    object: &surge_ts_syntax::ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    property_name: &str,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let object_result = evaluate_expression(object, object_span, symbols, ctx);

    let object_type = match object_result {
        InferredExpression::Known(ty) => ty,
        _ => return None, // already reported by evaluate_expression
    };

    if object_type.is_unknown() {
        return None;
    }

    let base_type = surge_ts_types::remove_undefined(&object_type);
    let base_type_name = base_type.name();

    match base_type {
        Type::Any => Some(Type::Any),
        Type::Unknown | Type::GenuineUnknown => None,
        Type::Array(element_type) if property_name == "map" => check_array_map_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        )
        .map(|ret| union_type(vec![ret, Type::Undefined])),
        Type::Array(element_type) if property_name == "find" => check_array_find_call(
            element_type.as_ref(),
            property_span,
            call_span,
            arguments,
            symbols,
            ctx,
        )
        .map(|ret| union_type(vec![ret, Type::Undefined])),
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in union_type.types() {
                if *ty == Type::Undefined {
                    result_types.push(Type::Undefined);
                    continue;
                }

                if property_name == "map"
                    && let Type::Array(element_type) = ty
                {
                    let mapped = check_array_map_call(
                        element_type.as_ref(),
                        property_span,
                        call_span,
                        arguments,
                        symbols,
                        ctx,
                    )?;
                    result_types.push(mapped);
                    continue;
                }

                let Some(property_type) = ty.get_property_access_type(property_name) else {
                    if no_lib_array_member(ty, ctx) {
                        result_types.push(Type::Any);
                        continue;
                    }
                    let diagnostic =
                        Diagnostic::ts2339(property_name, &base_type_name, ctx.file_name.clone());
                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        crate::spans::choose_span(property_span, object_span),
                    ));
                    return None;
                };

                let property_type_base = surge_ts_types::remove_undefined(&property_type);

                match callable_property_signature(property_type_base) {
                    Type::Function(function_type) => {
                        let return_type = check_function_type_call(
                            &function_type,
                            property_span,
                            call_span,
                            type_arguments,
                            arguments,
                            symbols,
                            ctx,
                        )?;
                        result_types.push(return_type);
                    }
                    Type::Any => result_types.push(Type::Any),
                    Type::Unknown | Type::GenuineUnknown => return None,
                    _ => {
                        ctx.push(diagnostic_with_syntax_span(
                            Diagnostic::ts2349(ctx.file_name.clone()),
                            crate::spans::choose_span(
                                call_span,
                                crate::spans::choose_span(property_span, object_span),
                            ),
                        ));
                        return None;
                    }
                }
            }

            Some(surge_ts_types::union_type(vec![
                surge_ts_types::union_type(result_types),
                Type::Undefined,
            ]))
        }
        _ => {
            if property_name == "map"
                && let Type::Array(element_type) = &base_type
            {
                return check_array_map_call(
                    element_type.as_ref(),
                    property_span,
                    call_span,
                    arguments,
                    symbols,
                    ctx,
                )
                .map(|ret| surge_ts_types::union_type(vec![ret, Type::Undefined]));
            }

            if property_name == "find"
                && let Type::Array(element_type) = &base_type
            {
                return check_array_find_call(
                    element_type.as_ref(),
                    property_span,
                    call_span,
                    arguments,
                    symbols,
                    ctx,
                )
                .map(|ret| surge_ts_types::union_type(vec![ret, Type::Undefined]));
            }

            let Some(property_type) = base_type.get_property_access_type(property_name) else {
                if no_lib_array_member(&base_type, ctx) {
                    return Some(surge_ts_types::union_type(vec![Type::Any, Type::Undefined]));
                }
                let diagnostic =
                    Diagnostic::ts2339(property_name, &base_type_name, ctx.file_name.clone());
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    crate::spans::choose_span(property_span, object_span),
                ));
                return None;
            };

            let property_type_base = surge_ts_types::remove_undefined(&property_type);

            match callable_property_signature(property_type_base) {
                Type::Function(function_type) => check_function_type_call(
                    &function_type,
                    property_span,
                    call_span,
                    type_arguments,
                    arguments,
                    symbols,
                    ctx,
                )
                .map(|ret| union_type(vec![ret, Type::Undefined])),
                Type::Any => Some(Type::Any),
                Type::Unknown | Type::GenuineUnknown => None,
                _ => {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2349(ctx.file_name.clone()),
                        crate::spans::choose_span(
                            call_span,
                            crate::spans::choose_span(property_span, object_span),
                        ),
                    ));
                    None
                }
            }
        }
    }
}

/// Under `noLib` the array member surface comes from the configured replacement
/// lib (roblox-ts's `Array` adds `size`/`push`/`pop`/… absent from the standard
/// JS array surface). surge collapses that interface to `Type::Array` for
/// assignability, discarding its member set, so a method the std surface does not
/// know is not a real typo here — resolve it permissively instead of emitting
/// TS2339. Without `noLib` the std array surface is authoritative.
/// A property typed as a callable object — a type literal carrying call
/// signatures, like expect-type's `toEqualTypeOf: { <E>(v: E): true; <E>(): true }`
/// — is invoked like a function. Surface its call signature so the property-call
/// match treats it as callable instead of a false TS2349.
fn callable_property_signature(ty: Type) -> Type {
    let signature = match &ty {
        Type::Object(object) => object.call_signature().cloned(),
        Type::Reference(_) => match ty.peeled() {
            Type::Object(object) => object.call_signature().cloned(),
            _ => None,
        },
        _ => None,
    };
    match signature {
        Some(signature) => Type::Function(signature),
        None => ty,
    }
}

fn no_lib_array_member(object_type: &Type, ctx: &CheckerContext) -> bool {
    ctx.options.no_lib && matches!(object_type, Type::Array(_))
}
