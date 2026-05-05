use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedCall, ParsedCallArgument, ParsedType, TextSpan as SyntaxTextSpan,
};
use typescript_rust_types::{FunctionType, Type, is_assignable_to, union_type};

use super::emit_type_only_as_value_diagnostic;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, infer_expression};
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::SymbolTable;

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

pub(crate) fn check_property_call_like(
    object_name: &str,
    object_span: Option<SyntaxTextSpan>,
    property_name: &str,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(symbol) = symbols.get(object_name).cloned() else {
        if emit_type_only_as_value_diagnostic(object_name, object_span, ctx) {
            return None;
        }

        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2304(object_name, ctx.file_name.clone()),
            object_span,
        ));
        return None;
    };

    let object_type_name = symbol.ty.name();

    match symbol.ty {
        Type::Any => Some(Type::Any),
        Type::Unknown => None,
        Type::Object(object_type) => {
            let Some(property_type) = object_type.get_property_access_type(property_name) else {
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
                Type::Union(_) => {
                    ctx.push(diagnostic_with_syntax_span(
                        Diagnostic::ts2349(ctx.file_name.clone()),
                        crate::spans::choose_span(
                            call_span,
                            crate::spans::choose_span(property_span, object_span),
                        ),
                    ));
                    None
                }
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
        _ => {
            let diagnostic =
                Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone());
            ctx.push(diagnostic_with_syntax_span(
                diagnostic,
                crate::spans::choose_span(property_span, object_span),
            ));
            None
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
        Type::Object(object_type) => {
            let Some(property_type) = object_type.get_property_access_type(property_name) else {
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
                    println!(
                        "TS2349 because property_type_base is: {:?}",
                        property_type_base
                    );
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
        _ => {
            let diagnostic =
                Diagnostic::ts2339(property_name, &base_type_name, ctx.file_name.clone());
            ctx.push(diagnostic_with_syntax_span(
                diagnostic,
                crate::spans::choose_span(property_span, object_span),
            ));
            None
        }
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

pub(crate) fn check_function_type_call(
    function_type: &FunctionType,
    callee_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    _type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let expected = function_type.parameters.len();
    let actual = arguments.len();
    let mut has_unresolved_argument = false;

    if expected != actual && !function_type.is_variadic {
        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2554(expected, actual, ctx.file_name.clone()),
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
