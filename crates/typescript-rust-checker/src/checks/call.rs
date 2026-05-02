use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{ParsedCall, ParsedCallArgument, TextSpan as SyntaxTextSpan};
use typescript_rust_types::{FunctionType, Type, is_assignable_to};

use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::context::{CheckerContext, convert_span};
use crate::infer::InferredExpression;
use crate::symbols::SymbolTable;

pub(crate) fn check_call(call: ParsedCall, ctx: &mut CheckerContext) {
    let symbols = ctx.symbols.clone();
    let _ = check_call_like(
        &call.callee_name,
        call.callee_span,
        &call.arguments,
        &symbols,
        ctx,
    );
}

pub(crate) fn check_call_like(
    callee_name: &str,
    callee_span: Option<SyntaxTextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(symbol) = symbols.get(callee_name).cloned() else {
        let diagnostic = Diagnostic::ts2304(callee_name, ctx.file_name.clone());
        let diagnostic = match callee_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
        return None;
    };

    let Type::Function(function_type) = symbol.ty else {
        let diagnostic = Diagnostic::ts2349(ctx.file_name.clone());
        let diagnostic = match callee_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
        return None;
    };

    check_function_type_call(&function_type, callee_span, arguments, symbols, ctx)
}

pub(crate) fn check_property_call_like(
    object_name: &str,
    object_span: Option<SyntaxTextSpan>,
    property_name: &str,
    property_span: Option<SyntaxTextSpan>,
    call_span: Option<SyntaxTextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Some(symbol) = symbols.get(object_name).cloned() else {
        let diagnostic = Diagnostic::ts2304(object_name, ctx.file_name.clone());
        let diagnostic = match object_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
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
                let diagnostic = match property_span.or(object_span) {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };
                ctx.push(diagnostic);
                return None;
            };

            match property_type {
                Type::Function(function_type) => check_function_type_call(
                    &function_type,
                    call_span.or(property_span),
                    arguments,
                    symbols,
                    ctx,
                ),
                Type::Any => Some(Type::Any),
                Type::Unknown => None,
                Type::Union(_) => {
                    let diagnostic = Diagnostic::ts2349(ctx.file_name.clone());
                    let diagnostic = match call_span.or(property_span).or(object_span) {
                        Some(span) => diagnostic.with_span(convert_span(span)),
                        None => diagnostic,
                    };
                    ctx.push(diagnostic);
                    None
                }
                _ => {
                    let diagnostic = Diagnostic::ts2349(ctx.file_name.clone());
                    let diagnostic = match call_span.or(property_span).or(object_span) {
                        Some(span) => diagnostic.with_span(convert_span(span)),
                        None => diagnostic,
                    };
                    ctx.push(diagnostic);
                    None
                }
            }
        }
        _ => {
            let diagnostic =
                Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone());
            let diagnostic = match property_span.or(object_span) {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            };
            ctx.push(diagnostic);
            None
        }
    }
}

pub(crate) fn check_function_type_call(
    function_type: &FunctionType,
    callee_span: Option<SyntaxTextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let expected = function_type.parameters.len();
    let actual = arguments.len();
    let mut has_unresolved_argument = false;

    if expected != actual {
        let diagnostic = Diagnostic::ts2554(expected, actual, ctx.file_name.clone());
        let diagnostic = match callee_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
        return None;
    }

    for (argument, parameter_type) in arguments.iter().zip(function_type.parameters.iter()) {
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

                    let diagnostic = match argument.span {
                        Some(span) => diagnostic.with_span(convert_span(span)),
                        None => diagnostic,
                    };

                    ctx.push(diagnostic);
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
