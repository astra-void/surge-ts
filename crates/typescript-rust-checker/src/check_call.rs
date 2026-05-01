use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{ParsedCall, ParsedCallArgument, TextSpan as SyntaxTextSpan};
use typescript_rust_types::{Type, is_assignable_to};

use crate::check_expr::{ExpectedTypeDiagnostic, evaluate_expression_with_expected_type};
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

    let expected = function_type.parameters.len();
    let actual = arguments.len();

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
            InferredExpression::UnresolvedIdentifier { .. } => {}
            InferredExpression::MissingProperty { .. } => {}
            InferredExpression::Unknown => {}
        }
    }

    Some(*function_type.return_type)
}
