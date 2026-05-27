use std::borrow::Cow;
use std::collections::BTreeMap;
use std::time::Instant;
use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedCall, ParsedCallArgument, ParsedExpression, ParsedObjectType, ParsedType,
    TextSpan as SyntaxTextSpan,
};
use typescript_rust_types::{
    FunctionType, Type, TypeCopyReason, is_assignable_to, union_type, with_type_copy_reason,
};

use super::emit_type_only_as_value_diagnostic;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::arena::{alloc_function_type, alloc_object_type};
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, infer_expression};
use crate::infer::{TypeParameterSubstitution, map_parsed_type_with_substitution};
use crate::program::{
    record_call_resolution, record_generic_call_inference_attempt,
    record_generic_call_inference_candidate, record_generic_call_inference_explicit_type_args_skip,
    record_generic_call_inference_failed, record_generic_call_inference_success,
    record_generic_call_inference_tuple_return_suppressed,
    record_generic_call_inference_unresolved_argument_skip, record_program_timing,
};
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::{FunctionSignatureInfo, SymbolTable};
pub(crate) fn check_call(call: ParsedCall, ctx: &mut CheckerContext) {
    let symbols = ctx
        .symbols
        .clone_with_reason(TypeCopyReason::CallResolution);
    let _ = check_call_like(
        &call.callee_name,
        call.callee_span,
        call.span,
        &call.type_arguments,
        &call.arguments,
        &symbols,
        ctx,
    );
}

pub(crate) fn check_call_like(
    callee_name: &str,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    record_call_resolution();
    let call_start = Instant::now();
    let Some(symbol) = symbols.get(callee_name) else {
        if emit_type_only_as_value_diagnostic(callee_name, callee_span, ctx) {
            return None;
        }

        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2304(callee_name, ctx.file_name.clone()),
            callee_span,
        ));
        return None;
    };

    if matches!(symbol.ty, Type::Unknown) {
        return None;
    }

    let Type::Function(function_type) = &symbol.ty else {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2349(ctx.file_name.clone()),
            callee_span,
        ));
        return None;
    };

    let (function_type, result) = with_type_copy_reason(TypeCopyReason::CallResolution, || {
        let function_type = instantiate_function_type(
            function_type,
            symbol.function_signature.as_ref(),
            type_arguments,
            arguments,
            symbols,
            ctx,
        );

        let result = check_function_type_call(
            function_type.as_ref(),
            callee_span,
            call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        );
        (function_type, result)
    });
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.call_expression_checking += call_start.elapsed()
    });
    let _ = function_type;
    result
}

pub(crate) fn check_new_like(
    callee: &ParsedExpression,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    if let ParsedExpression::Identifier { name, .. } = callee
        && let Some(result_type) =
            typescript_rust_types::Type::builtin_constructor_result_type(name)
    {
        for argument in arguments {
            let _ = evaluate_expression(&argument.expression, argument.span, symbols, ctx);
        }
        return Some(result_type);
    }

    let callee_result = evaluate_expression(callee, callee_span, symbols, ctx);
    let callee_type = match callee_result {
        InferredExpression::Known(ty) => ty,
        _ => return None,
    };

    match callee_type {
        Type::Function(function_type) => check_function_type_call(
            &function_type,
            callee_span,
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
                Diagnostic::ts2351(ctx.file_name.clone()),
                callee_span,
            ));
            None
        }
    }
}

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

            Some(typescript_rust_types::union_type(result_types))
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
    object: &typescript_rust_syntax::ParsedExpression,
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

    let base_type = typescript_rust_types::remove_undefined(&object_type);
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

                let property_type_base = typescript_rust_types::remove_undefined(&property_type);

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

            Some(typescript_rust_types::union_type(vec![
                typescript_rust_types::union_type(result_types),
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
                .map(|ret| typescript_rust_types::union_type(vec![ret, Type::Undefined]));
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
                .map(|ret| typescript_rust_types::union_type(vec![ret, Type::Undefined]));
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

            let property_type_base = typescript_rust_types::remove_undefined(&property_type);

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

fn check_array_map_call(
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
        vec![with_type_copy_reason(
            TypeCopyReason::PropertyCallResolution,
            || element_type.clone(),
        )],
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

fn check_array_find_call(
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
        vec![with_type_copy_reason(
            TypeCopyReason::PropertyCallResolution,
            || element_type.clone(),
        )],
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
        InferredExpression::Known(Type::Function(_)) => {
            Some(typescript_rust_types::union_type(vec![
                with_type_copy_reason(TypeCopyReason::PropertyCallResolution, || {
                    element_type.clone()
                }),
                Type::Undefined,
            ]))
        }
        InferredExpression::Known(Type::Any) => Some(Type::Any),
        InferredExpression::Known(Type::Unknown) => None,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => None,
        InferredExpression::Known(other) => Some(other),
    }
}

fn is_promise_all_receiver(object_type: &Type) -> bool {
    match object_type {
        Type::Object(object) => {
            object.contains_property("resolve") && object.contains_property("all")
        }
        _ => false,
    }
}

fn check_promise_all_call(
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
                typescript_rust_types::union_type(elements)
            })))
        }
        InferredExpression::Known(Type::Any) => Some(Type::Array(Box::new(Type::Any))),
        InferredExpression::Known(ty) => Some(Type::Array(Box::new(ty))),
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => Some(Type::Array(Box::new(Type::Any))),
    }
}

pub(crate) fn check_optional_call_like(
    callee: &typescript_rust_syntax::ParsedExpression,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let callee_result = evaluate_expression(callee, callee_span, symbols, ctx);

    let callee_type = match callee_result {
        InferredExpression::Known(ty) => ty,
        _ => return None,
    };

    if callee_type == Type::Unknown {
        return None;
    }

    let base_type = typescript_rust_types::remove_undefined(&callee_type);

    match base_type {
        Type::Any => Some(Type::Any),
        Type::Unknown => None,
        Type::Function(function_type) => check_function_type_call(
            &function_type,
            callee_span,
            call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        )
        .map(|ret| union_type(vec![ret, Type::Undefined])),
        _ => {
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2349(ctx.file_name.clone()),
                callee_span,
            ));
            None
        }
    }
}

fn instantiate_function_type<'a>(
    function_type: &'a FunctionType,
    function_signature: Option<&FunctionSignatureInfo>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Cow<'a, FunctionType> {
    let Some(function_signature) = function_signature else {
        return Cow::Borrowed(function_type);
    };

    if function_signature.type_parameters.is_empty() {
        return Cow::Borrowed(function_type);
    }

    record_generic_call_inference_attempt();

    if !type_arguments.is_empty() {
        record_generic_call_inference_explicit_type_args_skip();
        let substitution =
            explicit_type_argument_substitution(function_signature, type_arguments, ctx);
        return instantiate_function_type_with_substitution(
            function_type,
            function_signature,
            &substitution,
            false,
            ctx,
        );
    }

    let substitution =
        infer_type_argument_substitution(function_signature, arguments, symbols, ctx);

    if substitution
        .iter()
        .all(|(_, candidate)| *candidate == Type::Unknown)
    {
        record_generic_call_inference_failed();
        return Cow::Borrowed(function_type);
    }

    record_generic_call_inference_success();
    instantiate_function_type_with_substitution(
        function_type,
        function_signature,
        &substitution,
        true,
        ctx,
    )
}

fn instantiate_function_type_with_substitution<'a>(
    function_type: &'a FunctionType,
    function_signature: &FunctionSignatureInfo,
    substitution: &TypeParameterSubstitution,
    suppress_tuple_return_type: bool,
    ctx: &mut CheckerContext,
) -> Cow<'a, FunctionType> {
    with_type_copy_reason(TypeCopyReason::CallResolution, || {
        let mut instantiated_parameters = Vec::with_capacity(function_type.parameters().len());
        for (index, parameter) in function_type.parameters().iter().enumerate() {
            let Some(parsed_parameter) = function_signature
                .parameter_types
                .get(index)
                .and_then(|ty| ty.clone())
            else {
                instantiated_parameters.push(parameter.clone());
                continue;
            };

            instantiated_parameters.push(map_parsed_type_with_substitution(
                parsed_parameter,
                ctx,
                substitution,
            ));
        }

        let mut instantiated_return_type = function_signature
            .return_type
            .as_ref()
            .map(|return_type| {
                map_parsed_type_with_substitution(return_type.clone(), ctx, substitution)
            })
            .unwrap_or_else(|| function_type.return_type().clone());

        if suppress_tuple_return_type && matches!(instantiated_return_type, Type::Tuple(_)) {
            record_generic_call_inference_tuple_return_suppressed();
            instantiated_return_type = Type::Unknown;
        }

        Cow::Owned(alloc_function_type(
            instantiated_parameters,
            instantiated_return_type,
            function_type.is_variadic(),
            function_type.required_parameter_count(),
        ))
    })
}

pub(crate) fn instantiate_function_return_type_for_call(
    function_type: &FunctionType,
    function_signature: Option<&FunctionSignatureInfo>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    with_type_copy_reason(TypeCopyReason::CallResolution, || {
        instantiate_function_type(
            function_type,
            function_signature,
            type_arguments,
            arguments,
            symbols,
            ctx,
        )
        .return_type()
        .clone()
    })
}

fn explicit_type_argument_substitution(
    function_signature: &FunctionSignatureInfo,
    type_arguments: &[ParsedType],
    ctx: &mut CheckerContext,
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();
    for (type_parameter, type_argument) in function_signature
        .type_parameters
        .iter()
        .zip(type_arguments.iter())
    {
        substitution.insert(
            type_parameter.name.clone(),
            map_parsed_type_with_substitution(
                type_argument.clone(),
                ctx,
                &TypeParameterSubstitution::new(),
            ),
        );
    }
    substitution
}

fn infer_type_argument_substitution(
    function_signature: &FunctionSignatureInfo,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();
    for type_parameter in &function_signature.type_parameters {
        substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
    }

    for (index, argument) in arguments.iter().enumerate() {
        let Some(parameter_type) = function_signature
            .parameter_types
            .get(index)
            .and_then(|ty| ty.as_ref())
        else {
            continue;
        };

        let inferred_argument = infer_expression(&argument.expression, symbols, ctx);
        let InferredExpression::Known(argument_type) = inferred_argument else {
            record_generic_call_inference_unresolved_argument_skip();
            continue;
        };

        if argument_type == Type::Unknown || type_contains_unknown(&argument_type) {
            record_generic_call_inference_unresolved_argument_skip();
            continue;
        }

        collect_inferred_type_argument(parameter_type, &argument_type, &mut substitution, false);
    }

    substitution
}

fn collect_inferred_type_argument(
    parameter_type: &ParsedType,
    argument_type: &Type,
    substitution: &mut TypeParameterSubstitution,
    widen_literals: bool,
) {
    if type_contains_unknown(argument_type) {
        return;
    }

    match parameter_type {
        ParsedType::Named(named_type) => {
            record_type_argument_candidate(
                substitution,
                &named_type.name,
                argument_type,
                widen_literals,
            );
        }
        ParsedType::Array(element_type) => match argument_type {
            Type::Array(actual_element_type) => {
                collect_inferred_type_argument(
                    element_type.as_ref(),
                    actual_element_type.as_ref(),
                    substitution,
                    true,
                );
            }
            Type::Tuple(elements) => {
                for element in elements {
                    collect_inferred_type_argument(
                        element_type.as_ref(),
                        element,
                        substitution,
                        true,
                    );
                }
            }
            _ => {}
        },
        ParsedType::Tuple(expected_elements) => {
            if let Type::Tuple(actual_elements) = argument_type
                && expected_elements.len() == actual_elements.len()
            {
                for (expected_element, actual_element) in
                    expected_elements.iter().zip(actual_elements.iter())
                {
                    collect_inferred_type_argument(
                        expected_element,
                        actual_element,
                        substitution,
                        true,
                    );
                }
            }
        }
        ParsedType::Object(expected_object_type) => {
            if let Type::Object(actual_object_type) = argument_type {
                collect_object_type_candidates(
                    expected_object_type,
                    actual_object_type,
                    substitution,
                );
            }
        }
        ParsedType::Union(expected_types) => {
            if let Type::Union(actual_union) = argument_type
                && expected_types.len() == actual_union.types().len()
            {
                for (expected_element, actual_element) in
                    expected_types.iter().zip(actual_union.types().iter())
                {
                    collect_inferred_type_argument(
                        expected_element,
                        actual_element,
                        substitution,
                        widen_literals,
                    );
                }
            }
        }
        _ => {}
    }
}

fn collect_object_type_candidates(
    expected_object_type: &ParsedObjectType,
    actual_object_type: &typescript_rust_types::ObjectType,
    substitution: &mut TypeParameterSubstitution,
) {
    for property in &expected_object_type.properties {
        let Some(actual_property_type) =
            actual_object_type.get_property_access_type(&property.name)
        else {
            continue;
        };

        collect_inferred_type_argument(&property.ty, &actual_property_type, substitution, true);
    }
}

fn record_type_argument_candidate(
    substitution: &mut TypeParameterSubstitution,
    type_parameter_name: &str,
    argument_type: &Type,
    widen_literals: bool,
) {
    let Some(existing) = substitution.get(type_parameter_name).cloned() else {
        return;
    };

    record_generic_call_inference_candidate();
    let candidate = if widen_literals {
        widen_candidate_type(argument_type)
    } else {
        with_type_copy_reason(TypeCopyReason::CallResolution, || argument_type.clone())
    };

    if existing == Type::Unknown {
        substitution.set(type_parameter_name.to_string(), candidate, false);
        return;
    }

    if existing == candidate {
        return;
    }

    if let Some(common_primitive) = common_primitive_candidate(&existing, &candidate) {
        substitution.set(type_parameter_name.to_string(), common_primitive, false);
    }
}

fn common_primitive_candidate(existing: &Type, candidate: &Type) -> Option<Type> {
    match (existing.base_primitive(), candidate.base_primitive()) {
        (Some(Type::String), Some(Type::String)) => Some(Type::String),
        (Some(Type::Number), Some(Type::Number)) => Some(Type::Number),
        (Some(Type::Boolean), Some(Type::Boolean)) => Some(Type::Boolean),
        _ => None,
    }
}

fn widen_candidate_type(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        Type::Array(element) => Type::Array(Box::new(widen_candidate_type(element.as_ref()))),
        Type::Tuple(elements) => Type::Tuple(elements.iter().map(widen_candidate_type).collect()),
        Type::Object(object) => {
            let properties = object
                .properties
                .iter()
                .map(|(name, property)| {
                    (
                        name.clone(),
                        typescript_rust_types::ObjectProperty {
                            ty: widen_candidate_type(&property.ty),
                            optional: property.optional,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();

            Type::Object(alloc_object_type(properties, None))
        }
        Type::Union(union_payload) => typescript_rust_types::union_type(
            union_payload
                .types()
                .iter()
                .map(widen_candidate_type)
                .collect(),
        ),
        _ => with_type_copy_reason(TypeCopyReason::CallResolution, || ty.clone()),
    }
}

pub(crate) fn check_function_type_call(
    function_type: &FunctionType,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    _type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let required = function_type.required_parameter_count();
    let expected = function_type.parameters().len();
    let actual = arguments.len();
    let mut has_unresolved_argument = false;

    if actual < required || (!function_type.is_variadic() && actual > expected) {
        let expected_count = if actual < required {
            required
        } else {
            expected
        };
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2554(expected_count, actual, ctx.file_name.clone()),
            call_span.or(callee_span),
        ));
        return None;
    }

    for (i, argument) in arguments.iter().enumerate() {
        let parameter_type = if i < expected {
            &function_type.parameters()[i]
        } else if function_type.is_variadic() && expected > 0 {
            &function_type.parameters()[expected - 1]
        } else {
            &Type::Any
        };

        let inferred_argument = evaluate_expression_with_expected_type(
            &argument.expression,
            argument.span,
            Some(parameter_type),
            ExpectedTypeDiagnostic::ArgumentNotAssignable,
            symbols,
            ctx,
        );

        match inferred_argument {
            InferredExpression::Known(argument_type) => {
                if argument_type == Type::Unknown {
                    continue;
                }

                if !type_contains_unknown(parameter_type)
                    && !type_contains_unknown(&argument_type)
                    && !is_assignable_to(&argument_type, parameter_type)
                {
                    let argument_type_name = argument_type.name();
                    let parameter_type_name = parameter_type.name();
                    let diagnostic = Diagnostic::ts2345(
                        &argument_type_name,
                        &parameter_type_name,
                        ctx.file_name.clone(),
                    );

                    ctx.push(diagnostic_with_syntax_span(diagnostic, argument.span));
                }
            }
            InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. } => {
                has_unresolved_argument = true;
            }
            InferredExpression::Unknown => {}
        }
    }

    if has_unresolved_argument {
        return None;
    }

    Some(with_type_copy_reason(
        TypeCopyReason::CallResolution,
        || function_type.return_type().clone(),
    ))
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
