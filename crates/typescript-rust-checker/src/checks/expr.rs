use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{ParsedExpression, TextSpan as SyntaxTextSpan};
use typescript_rust_types::{NumberLiteralType, Type, is_assignable_to, union_type};

use super::call::{
    check_call_like, check_optional_call_like, check_optional_property_call,
    check_property_call_like,
};
use super::emit_type_only_as_value_diagnostic;
use super::ops;
use crate::context::CheckerContext;
use crate::infer::{InferredExpression, infer_expression};
use crate::spans::{choose_span, diagnostic_with_syntax_span};
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
        ParsedExpression::ArrayLiteral { elements, .. } => {
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
            ..
        } => match check_call_like(
            callee_name,
            *callee_span,
            None,
            &[],
            arguments,
            symbols,
            ctx,
        ) {
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
            ..
        } => match check_property_call_like(
            object_name,
            *object_span,
            property_name,
            *property_span,
            *call_span,
            &[],
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::OptionalPropertyCall {
            object,
            object_span,
            property_name,
            property_span,
            call_span,
            type_arguments,
            arguments,
        } => match check_optional_property_call(
            object,
            *object_span,
            property_name,
            *property_span,
            *call_span,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::OptionalCall {
            callee,
            callee_span,
            type_arguments,
            arguments,
        } => match check_optional_call_like(
            callee,
            *callee_span,
            None,
            type_arguments,
            arguments,
            symbols,
            ctx,
        ) {
            Some(return_type) => InferredExpression::Known(return_type),
            None => InferredExpression::Unknown,
        },
        ParsedExpression::NullishCoalescing {
            left,
            left_span,
            right,
            right_span,
        } => {
            let left_result = evaluate_expression(left, left_span.or(fallback_span), symbols, ctx);
            let right_result =
                evaluate_expression(right, right_span.or(fallback_span), symbols, ctx);

            match (left_result, right_result) {
                (InferredExpression::Known(left_type), InferredExpression::Known(right_type)) => {
                    if left_type == Type::Any || left_type == Type::Unknown {
                        InferredExpression::Known(left_type)
                    } else if left_type == Type::Undefined {
                        InferredExpression::Known(right_type)
                    } else {
                        let filtered_left = typescript_rust_types::remove_undefined(&left_type);
                        InferredExpression::Known(union_type(vec![filtered_left, right_type]))
                    }
                }
                (InferredExpression::Known(Type::Unknown) | InferredExpression::Unknown, _)
                | (_, InferredExpression::Known(Type::Unknown) | InferredExpression::Unknown) => {
                    InferredExpression::Unknown
                }
                _ => InferredExpression::Unknown,
            }
        }
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
        ParsedExpression::OptionalPropertyAccess {
            object,
            object_span,
            property_name,
            property_span,
        } => {
            let _ = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);
            let inferred_expression = infer_expression(expression, symbols);
            if let InferredExpression::MissingProperty {
                property_name,
                object_type,
                span,
            } = &inferred_expression
            {
                let object_type_name = object_type.name();
                ctx.push(diagnostic_with_syntax_span(
                    Diagnostic::ts2339(property_name, &object_type_name, ctx.file_name.clone()),
                    choose_span(*span, fallback_span),
                ));
            }
            inferred_expression
        }
        ParsedExpression::OptionalIndexAccess {
            object,
            object_span,
            index,
            index_span,
        } => evaluate_optional_index_access(
            object,
            *object_span,
            index,
            *index_span,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => evaluate_index_access(
            object_name,
            *object_span,
            index,
            *index_span,
            fallback_span,
            symbols,
            ctx,
        ),
        ParsedExpression::TypeAssertion {
            expression: asserted_expression,
            expression_span,
            ty,
            type_span,
        } => {
            // Evaluate the inner expression so it participates in checking
            let _ = evaluate_expression(
                asserted_expression,
                expression_span.or(fallback_span),
                symbols,
                ctx,
            );

            // Resolve the target type
            let resolved_type = crate::infer::map_parsed_type(ty.clone(), ctx);

            // If the type is unresolved (e.g. unknown named type), map_parsed_type
            // already emits TS2304 and returns Type::Unknown.
            // We just return it as the assertion result.
            InferredExpression::Known(resolved_type)
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
            if emit_type_only_as_value_diagnostic(&name, span, ctx) {
                return;
            }

            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2304(&name, ctx.file_name.clone()),
                choose_span(span, fallback_span),
            ));
        }
        InferredExpression::MissingProperty {
            property_name,
            object_type,
            span,
        } => {
            let object_type_name = object_type.name();
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2339(&property_name, &object_type_name, ctx.file_name.clone()),
                choose_span(span, fallback_span),
            ));
        }
        InferredExpression::Unknown => {}
    }
}

fn evaluate_optional_index_access(
    object: &ParsedExpression,
    object_span: Option<SyntaxTextSpan>,
    index: &ParsedExpression,
    index_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let object_result = evaluate_expression(object, object_span.or(fallback_span), symbols, ctx);

    let object_type = match object_result {
        InferredExpression::Known(ty) => ty,
        InferredExpression::UnresolvedIdentifier { name, span } => {
            return InferredExpression::UnresolvedIdentifier { name, span };
        }
        InferredExpression::MissingProperty { .. } | InferredExpression::Unknown => {
            return InferredExpression::Unknown;
        }
    };

    let base_type = typescript_rust_types::remove_undefined(&object_type);

    match base_type {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Some(index_value) = crate::infer::tuple_index_value(&index_type) {
                return match elements.get(index_value).cloned() {
                    Some(element_type) => {
                        InferredExpression::Known(union_type(vec![element_type, Type::Undefined]))
                    }
                    None => {
                        let index_type_name = index_type.name();
                        let object_type_name = Type::Tuple(elements.to_vec()).name();
                        let diagnostic = Diagnostic::ts2339(
                            &index_type_name,
                            &object_type_name,
                            ctx.file_name.clone(),
                        );

                        ctx.push(diagnostic_with_syntax_span(
                            diagnostic,
                            choose_span(index_span, choose_span(object_span, fallback_span)),
                        ));
                        InferredExpression::Unknown
                    }
                };
            }

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(vec![
                union_type(elements.to_vec()),
                Type::Undefined,
            ]))
        }
        Type::Array(element_type) => {
            if matches!(element_type.as_ref(), Type::Unknown) {
                return InferredExpression::Unknown;
            }

            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(vec![
                element_type.as_ref().clone(),
                Type::Undefined,
            ]))
        }
        _ => InferredExpression::Unknown,
    }
}

fn evaluate_index_access(
    object_name: &str,
    object_span: Option<SyntaxTextSpan>,
    index: &ParsedExpression,
    index_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(symbol) = symbols.get(object_name).cloned() else {
        if emit_type_only_as_value_diagnostic(object_name, object_span, ctx) {
            return InferredExpression::Unknown;
        }

        ctx.push(diagnostic_with_syntax_span(
            Diagnostic::ts2304(object_name, ctx.file_name.clone()),
            choose_span(object_span, fallback_span),
        ));
        return InferredExpression::Unknown;
    };

    match symbol.ty {
        Type::Any => InferredExpression::Known(Type::Any),
        Type::Unknown => InferredExpression::Unknown,
        Type::Tuple(elements) => {
            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if let Some(index_value) = tuple_index_value(&index_type) {
                return match elements.get(index_value).cloned() {
                    Some(element_type) => InferredExpression::Known(element_type),
                    None => {
                        let index_type_name = index_type.name();
                        let object_type_name = Type::Tuple(elements.to_vec()).name();
                        let diagnostic = Diagnostic::ts2339(
                            &index_type_name,
                            &object_type_name,
                            ctx.file_name.clone(),
                        );

                        ctx.push(diagnostic_with_syntax_span(
                            diagnostic,
                            choose_span(index_span, choose_span(object_span, fallback_span)),
                        ));
                        InferredExpression::Unknown
                    }
                };
            }

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known(union_type(elements.to_vec()))
        }
        Type::Array(element_type) => {
            if matches!(element_type.as_ref(), Type::Unknown) {
                return InferredExpression::Unknown;
            }

            let index_result =
                evaluate_expression(index, index_span.or(fallback_span), symbols, ctx);
            let index_type = match index_result {
                InferredExpression::Known(ty) => ty,
                InferredExpression::UnresolvedIdentifier { .. }
                | InferredExpression::MissingProperty { .. }
                | InferredExpression::Unknown => return InferredExpression::Unknown,
            };

            if !is_assignable_to(&index_type, &Type::Number) {
                let index_type_name = index_type.name();
                let expected_type_name = Type::Number.name();
                let diagnostic = Diagnostic::ts2322(
                    &index_type_name,
                    &expected_type_name,
                    ctx.file_name.clone(),
                );

                ctx.push(diagnostic_with_syntax_span(
                    diagnostic,
                    choose_span(index_span, choose_span(object_span, fallback_span)),
                ));
                return InferredExpression::Unknown;
            }

            InferredExpression::Known((*element_type).clone())
        }
        Type::Object(_)
        | Type::Function(_)
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
            ctx.push(diagnostic_with_syntax_span(
                Diagnostic::ts2339(object_name, &object_type_name, ctx.file_name.clone()),
                choose_span(object_span, fallback_span),
            ));
            InferredExpression::Unknown
        }
    }
}

fn tuple_index_value(index_type: &Type) -> Option<usize> {
    let Type::NumberLiteral(NumberLiteralType { value }) = index_type else {
        return None;
    };

    value.parse::<usize>().ok()
}
