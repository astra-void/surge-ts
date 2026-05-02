use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{ParsedExpression, TextSpan as SyntaxTextSpan};
use typescript_rust_types::{Type, is_assignable_to};

use super::call::{check_call_like, check_property_call_like};
use super::ops;
use crate::context::{CheckerContext, convert_span};
use crate::infer::{InferredExpression, infer_expression};
use crate::symbols::SymbolTable;

pub(crate) fn check_expression_statement(expression: ParsedExpression, ctx: &mut CheckerContext) {
    let symbols = ctx.symbols.clone();
    let _ = evaluate_expression(&expression, None, &symbols, ctx);
}

pub(crate) fn evaluate_expression(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match expression {
        ParsedExpression::ArrayLiteral(elements) => {
            let inferred_expression = infer_expression(expression, symbols);

            for element in elements {
                let _ = evaluate_expression(
                    &element.expression,
                    element.span.or(fallback_span),
                    symbols,
                    ctx,
                );
            }

            report_inferred_expression(inferred_expression.clone(), fallback_span, ctx);
            inferred_expression
        }
        ParsedExpression::Call {
            callee_name,
            callee_span,
            arguments,
        } => match check_call_like(callee_name, *callee_span, arguments, symbols, ctx) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::PropertyCall {
            object_name,
            object_span,
            property_name,
            property_span,
            call_span,
            arguments,
        } => match check_property_call_like(
            object_name,
            *object_span,
            property_name,
            *property_span,
            *call_span,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::Logical {
            left,
            left_span,
            operator: _,
            operator_span: _,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            ops::evaluate_logical_expression(left_result, right_result)
        }
        ParsedExpression::Binary {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            ops::evaluate_binary_expression(
                left_result,
                right_result,
                *operator,
                *left_span,
                *operator_span,
                *right_span,
                fallback_span,
                ctx,
            )
        }
        ParsedExpression::Unary {
            operator,
            operand,
            operand_span,
            ..
        } => {
            let operand_result =
                evaluate_expression(operand, operand_span.or(fallback_span), symbols, ctx);

            ops::evaluate_unary_expression(
                *operator,
                operand_result,
                operand_span.or(fallback_span),
                ctx,
            )
        }
        ParsedExpression::Conditional {
            condition,
            condition_span,
            when_true,
            when_true_span,
            when_false,
            when_false_span,
        } => {
            let condition_result =
                evaluate_expression(condition, condition_span.or(fallback_span), symbols, ctx);
            let true_result =
                evaluate_expression(when_true, when_true_span.or(fallback_span), symbols, ctx);
            let false_result =
                evaluate_expression(when_false, when_false_span.or(fallback_span), symbols, ctx);

            ops::evaluate_conditional_expression(condition_result, true_result, false_result)
        }
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => {
            let Some(symbol) = symbols.get(object_name).cloned() else {
                let diagnostic = Diagnostic::ts2304(object_name, ctx.file_name.clone());
                let diagnostic = match object_span.or(fallback_span) {
                    Some(span) => diagnostic.with_span(convert_span(span)),
                    None => diagnostic,
                };
                ctx.push(diagnostic);
                return InferredExpression::Unknown;
            };

            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            match symbol.ty {
                Type::Any => InferredExpression::Known(Type::Any),
                Type::Unknown => InferredExpression::Unknown,
                Type::Array(element_type) => {
                    if !is_assignable_to(&index_type, &Type::Number) {
                        let index_type_name = index_type.name();
                        let expected_type_name = Type::Number.name();
                        let mut diagnostic = Diagnostic::ts2322(
                            &index_type_name,
                            &expected_type_name,
                            ctx.file_name.clone(),
                        );

                        if let Some(span) = index_span
                            .as_ref()
                            .copied()
                            .or(*object_span)
                            .or(fallback_span)
                        {
                            diagnostic = diagnostic.with_span(convert_span(span));
                        }

                        ctx.push(diagnostic);
                        return InferredExpression::Unknown;
                    }

                    InferredExpression::Known((*element_type).clone())
                }
                Type::Object(_) => {
                    let object_type_name = symbol.ty.name();
                    let mut diagnostic =
                        Diagnostic::ts2339(object_name, &object_type_name, ctx.file_name.clone());

                    if let Some(span) = object_span.or(fallback_span) {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }

                    ctx.push(diagnostic);
                    InferredExpression::Unknown
                }
                Type::Function(_)
                | Type::String
                | Type::Number
                | Type::Boolean
                | Type::Void
                | Type::StringLiteral(_)
                | Type::NumberLiteral(_)
                | Type::BooleanLiteral(_)
                | Type::Undefined
                | Type::Union(_) => {
                    let object_type_name = symbol.ty.name();
                    let mut diagnostic =
                        Diagnostic::ts2339(object_name, &object_type_name, ctx.file_name.clone());

                    if let Some(span) = object_span.or(fallback_span) {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }

                    ctx.push(diagnostic);
                    InferredExpression::Unknown
                }
            }
        }
        _ => {
            let inferred_expression = infer_expression(expression, symbols);
            report_inferred_expression(inferred_expression.clone(), fallback_span, ctx);
            inferred_expression
        }
    }
}

pub(crate) fn report_inferred_expression(
    inferred_expression: InferredExpression,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    match inferred_expression {
        InferredExpression::Known(known_type) => {
            if known_type == Type::Unknown {
                return;
            }
        }
        InferredExpression::UnresolvedIdentifier { name, span } => {
            let diagnostic_span = span.or(fallback_span);
            let mut diagnostic = Diagnostic::ts2304(&name, ctx.file_name.clone());

            if let Some(span) = diagnostic_span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }

            ctx.push(diagnostic);
        }
        InferredExpression::MissingProperty {
            property_name,
            object_type,
            span,
        } => {
            let diagnostic_span = span.or(fallback_span);
            let object_type_name = object_type.name();
            let mut diagnostic =
                Diagnostic::ts2339(&property_name, &object_type_name, ctx.file_name.clone());

            if let Some(span) = diagnostic_span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }

            ctx.push(diagnostic);
        }
        InferredExpression::Unknown => {}
    }
}
