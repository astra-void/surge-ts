use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedCall, ParsedCallArgument, ParsedExpression, ParsedType, TextSpan as SyntaxTextSpan,
};
use typescript_rust_types::{FunctionType, Type, is_assignable_to, union_type};

use super::emit_type_only_as_value_diagnostic;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::infer::{TypeParameterSubstitution, map_parsed_type_with_substitution};
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::{FunctionSignatureInfo, SymbolTable};

pub(crate) fn check_call(call: ParsedCall, ctx: &mut CheckerContext) {
    let symbols = ctx.symbols.clone();
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
    let Some(symbol) = symbols.get(callee_name).cloned() else {
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

    let Type::Function(function_type) = symbol.ty else {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2349(ctx.file_name.clone()),
            callee_span,
        ));
        return None;
    };

    let function_type = instantiate_function_type(
        &function_type,
        symbol.function_signature.as_ref(),
        type_arguments,
        ctx,
    );

    check_function_type_call(
        &function_type,
        callee_span,
        call_span,
        type_arguments,
        arguments,
        symbols,
        ctx,
    )
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
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in &union_type.types {
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
        Type::Union(union_type) => {
            let mut result_types = vec![];
            for ty in &union_type.types {
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

    let callback_type = Type::Function(FunctionType {
        parameters: vec![element_type.clone()],
        return_type: Box::new(Type::Any),
        is_variadic: false,
        required_parameter_count: 1,
    });

    let inferred_callback = evaluate_expression_with_expected_type(
        &arguments[0].expression,
        arguments[0].span,
        Some(&callback_type),
        ExpectedTypeDiagnostic::ArgumentNotAssignable,
        symbols,
        ctx,
    );

    match inferred_callback {
        InferredExpression::Known(Type::Function(function_type)) => {
            Some(Type::Array(Box::new((*function_type.return_type).clone())))
        }
        InferredExpression::Known(Type::Any) => Some(Type::Array(Box::new(Type::Any))),
        InferredExpression::Known(Type::Unknown) => None,
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => None,
        InferredExpression::Known(other) => Some(Type::Array(Box::new(other))),
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
        InferredExpression::Known(Type::Array(element_type)) => {
            Some(Type::Array(Box::new((*element_type).clone())))
        }
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

fn instantiate_function_type(
    function_type: &FunctionType,
    function_signature: Option<&FunctionSignatureInfo>,
    type_arguments: &[ParsedType],
    ctx: &mut CheckerContext,
) -> FunctionType {
    let Some(function_signature) = function_signature else {
        return function_type.clone();
    };

    if function_signature.type_parameters.is_empty() || type_arguments.is_empty() {
        return function_type.clone();
    }

    let mut substitution = TypeParameterSubstitution::new();
    for (type_parameter, type_argument) in function_signature
        .type_parameters
        .iter()
        .zip(type_arguments.iter())
    {
        let type_argument = type_argument.clone();
        substitution.insert(
            type_parameter.name.clone(),
            map_parsed_type_with_substitution(
                type_argument,
                ctx,
                &TypeParameterSubstitution::new(),
            ),
        );
    }

    let mut instantiated_parameters = Vec::with_capacity(function_type.parameters.len());
    for (index, parameter) in function_type.parameters.iter().enumerate() {
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
            &substitution,
        ));
    }

    let instantiated_return_type = function_signature
        .return_type
        .as_ref()
        .map(|return_type| {
            let return_type = return_type.clone();
            map_parsed_type_with_substitution(return_type, ctx, &substitution)
        })
        .unwrap_or_else(|| (*function_type.return_type).clone());

    FunctionType {
        parameters: instantiated_parameters,
        return_type: Box::new(instantiated_return_type),
        is_variadic: function_type.is_variadic,
        required_parameter_count: function_type.required_parameter_count,
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
    let required = function_type.required_parameter_count;
    let expected = function_type.parameters.len();
    let actual = arguments.len();
    let mut has_unresolved_argument = false;

    if actual < required || (!function_type.is_variadic && actual > expected) {
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
            &function_type.parameters[i]
        } else if function_type.is_variadic && expected > 0 {
            &function_type.parameters[expected - 1]
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

                if !is_assignable_to(&argument_type, parameter_type) {
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

    Some((*function_type.return_type).clone())
}
