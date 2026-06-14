use std::time::Instant;
use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedCall, ParsedCallArgument, ParsedExpression, ParsedType, TextSpan as SyntaxTextSpan,
};
use typescript_rust_types::{
    FunctionType, Type, TypeCopyReason, is_assignable_to, union_type, with_type_copy_reason,
};

use super::emit_type_only_as_value_diagnostic;
use super::expected::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
use crate::checks::expr::evaluate_expression;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::program::{record_call_resolution, record_program_timing};
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::SymbolTable;

mod builtins;
mod instantiate;
mod property;

pub(crate) use builtins::*;
pub(crate) use instantiate::*;
pub(crate) use property::*;
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
