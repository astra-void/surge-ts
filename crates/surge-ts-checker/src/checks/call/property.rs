//! Property-call and optional-property-call checking.

use super::*;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedCallArgument, ParsedExpression, ParsedType, TextSpan as SyntaxTextSpan,
};
use surge_ts_types::{Type, union_type};

use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
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

    match object_ty {
        Type::Any => Some(Type::Any),
        Type::Unknown => None,
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
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone()),
                        crate::spans::choose_span(property_span, object_span),
                    ));
                    return None;
                };

                match property_type {
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
                    Type::Unknown => return None,
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
                let diagnostic =
                    Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone());
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    crate::spans::choose_span(property_span, object_span),
                ));
                return None;
            };

            match property_type {
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
                Type::Unknown => None,
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

    if object_type == Type::Unknown {
        return None;
    }

    let base_type = surge_ts_types::remove_undefined(&object_type);
    let base_type_name = base_type.name();

    match base_type {
        Type::Any => Some(Type::Any),
        Type::Unknown => None,
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
                    let diagnostic =
                        Diagnostic::ts2339(property_name, &base_type_name, ctx.file_name.clone());
                    ctx.push(diagnostic_with_syntax_span(
                        diagnostic,
                        crate::spans::choose_span(property_span, object_span),
                    ));
                    return None;
                };

                let property_type_base = surge_ts_types::remove_undefined(&property_type);

                match property_type_base {
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
                    Type::Unknown => return None,
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
                let diagnostic =
                    Diagnostic::ts2339(property_name, &base_type_name, ctx.file_name.clone());
                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    crate::spans::choose_span(property_span, object_span),
                ));
                return None;
            };

            let property_type_base = surge_ts_types::remove_undefined(&property_type);

            match property_type_base {
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
                Type::Unknown => None,
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
