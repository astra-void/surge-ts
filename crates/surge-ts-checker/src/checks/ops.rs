use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedBinaryOperator, ParsedUnaryOperator, TextSpan as SyntaxTextSpan};
use surge_ts_types::{Type, union_type};

use crate::checks::expr::operand_display_name;
use crate::context::{CheckerContext, convert_span};
use crate::infer::InferredExpression;

pub(crate) fn evaluate_binary_expression(
    left_result: InferredExpression,
    right_result: InferredExpression,
    operator: ParsedBinaryOperator,
    left_span: Option<SyntaxTextSpan>,
    operator_span: Option<SyntaxTextSpan>,
    right_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match operator {
        ParsedBinaryOperator::Add => evaluate_add_binary(
            left_result,
            right_result,
            operator_span.or(fallback_span),
            ctx,
        ),
        ParsedBinaryOperator::Subtract
        | ParsedBinaryOperator::Multiply
        | ParsedBinaryOperator::Divide
        | ParsedBinaryOperator::Remainder
        | ParsedBinaryOperator::Exponential
        | ParsedBinaryOperator::ShiftLeft
        | ParsedBinaryOperator::ShiftRight
        | ParsedBinaryOperator::ShiftRightZeroFill
        | ParsedBinaryOperator::BitwiseAnd
        | ParsedBinaryOperator::BitwiseOR
        | ParsedBinaryOperator::BitwiseXOR => evaluate_arithmetic_binary(
            left_result,
            right_result,
            left_span.or(fallback_span),
            right_span.or(fallback_span),
            ctx,
        ),
        ParsedBinaryOperator::LessThan
        | ParsedBinaryOperator::LessThanEquals
        | ParsedBinaryOperator::GreaterThan
        | ParsedBinaryOperator::GreaterThanEquals => evaluate_comparison_binary(
            left_result,
            right_result,
            binary_operator_text(operator),
            operator_span.or(fallback_span),
            ctx,
        ),
        ParsedBinaryOperator::StrictEquals
        | ParsedBinaryOperator::StrictNotEquals
        | ParsedBinaryOperator::Equals
        | ParsedBinaryOperator::NotEquals => evaluate_equality_binary(
            left_result,
            right_result,
            operator_span.or(fallback_span),
            ctx,
        ),
        // `"prop" in obj` is a boolean property-presence test. The operands are
        // already evaluated by the caller; the result is simply `boolean`.
        ParsedBinaryOperator::In => InferredExpression::Known(Type::Boolean),
    }
}

pub(crate) fn evaluate_logical_expression(
    operator: surge_ts_syntax::ParsedLogicalOperator,
    left_result: InferredExpression,
    right_result: InferredExpression,
) -> InferredExpression {
    let (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty)) =
        (&left_result, &right_result)
    else {
        return InferredExpression::Unknown;
    };
    if *left_ty == Type::Unknown || *right_ty == Type::Unknown {
        return InferredExpression::Unknown;
    }

    // A logical expression yields one of its operand *values*, not `boolean`:
    // `a || b` is `NonNullable<a> | b` (the left's nullish branch is gone when it
    // falls through), and `a && b` is `a | b` (`a` when falsy, otherwise `b`).
    // `??` has its own handler. Modelling the operand union avoids false
    // assignability errors like `string | undefined || "x"` being treated as
    // `boolean`.
    let result = match operator {
        surge_ts_syntax::ParsedLogicalOperator::Or => surge_ts_types::union_type(vec![
            surge_ts_types::remove_nullish(left_ty),
            right_ty.clone(),
        ]),
        surge_ts_syntax::ParsedLogicalOperator::And => {
            surge_ts_types::union_type(vec![left_ty.clone(), right_ty.clone()])
        }
    };
    InferredExpression::Known(result)
}

pub(crate) fn evaluate_conditional_expression(
    condition_result: InferredExpression,
    true_result: InferredExpression,
    false_result: InferredExpression,
) -> InferredExpression {
    if !is_known_non_unknown(&condition_result) {
        return InferredExpression::Unknown;
    }

    let Some(true_type) = inferred_type(&true_result) else {
        return InferredExpression::Unknown;
    };
    let Some(false_type) = inferred_type(&false_result) else {
        return InferredExpression::Unknown;
    };

    if matches!(true_type, Type::Unknown) || matches!(false_type, Type::Unknown) {
        return InferredExpression::Unknown;
    }

    if matches!(true_type, Type::Any) || matches!(false_type, Type::Any) {
        return InferredExpression::Known(Type::Any);
    }

    if true_type == false_type {
        return InferredExpression::Known(true_type.clone());
    }

    InferredExpression::Known(union_type(vec![true_type.clone(), false_type.clone()]))
}

pub(crate) fn evaluate_unary_expression(
    operator: ParsedUnaryOperator,
    operand_result: InferredExpression,
    operand_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match operator {
        ParsedUnaryOperator::Not => {
            if is_known_non_unknown(&operand_result) {
                InferredExpression::Known(Type::Boolean)
            } else {
                InferredExpression::Unknown
            }
        }
        ParsedUnaryOperator::Plus | ParsedUnaryOperator::Minus => {
            let Some(operand_type) = inferred_type(&operand_result) else {
                return InferredExpression::Unknown;
            };

            if matches!(operand_type, Type::Unknown) {
                return InferredExpression::Unknown;
            }

            if matches!(operand_type, Type::Any) {
                return InferredExpression::Known(Type::Any);
            }

            if matches!(operand_type.base_primitive(), Some(Type::Number)) {
                return InferredExpression::Known(Type::Number);
            }

            let file_name = ctx.file_name.clone();
            push_diagnostic(ctx, Diagnostic::ts2356(file_name), operand_span);
            InferredExpression::Unknown
        }
    }
}

fn evaluate_add_binary(
    left_result: InferredExpression,
    right_result: InferredExpression,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(left_type) = inferred_type(&left_result) else {
        return InferredExpression::Unknown;
    };
    let Some(right_type) = inferred_type(&right_result) else {
        return InferredExpression::Unknown;
    };

    if matches!(left_type, Type::Unknown) || matches!(right_type, Type::Unknown) {
        return InferredExpression::Unknown;
    }

    if matches!(left_type, Type::Any) || matches!(right_type, Type::Any) {
        return InferredExpression::Known(Type::Any);
    }

    if is_string_like_for_add(&left_type) && is_string_like_for_add(&right_type) {
        return InferredExpression::Known(Type::String);
    }

    if is_string_like_for_add(&left_type) && is_number_like_for_add(&right_type) {
        return InferredExpression::Known(Type::String);
    }

    if is_number_like_for_add(&left_type) && is_string_like_for_add(&right_type) {
        return InferredExpression::Known(Type::String);
    }

    if is_number_like_for_add(&left_type) && is_number_like_for_add(&right_type) {
        return InferredExpression::Known(Type::Number);
    }

    let file_name = ctx.file_name.clone();
    push_diagnostic(
        ctx,
        Diagnostic::ts2365(
            "+",
            &operand_display_name(&left_type),
            &operand_display_name(&right_type),
            file_name,
        ),
        fallback_span,
    );
    InferredExpression::Unknown
}

fn is_string_like_for_add(ty: &Type) -> bool {
    match ty {
        Type::String | Type::StringLiteral(_) => true,
        Type::Union(union) => union.types().iter().all(is_string_like_for_add),
        _ => false,
    }
}

fn is_number_like_for_add(ty: &Type) -> bool {
    match ty {
        Type::Number | Type::NumberLiteral(_) => true,
        Type::Union(union) => union.types().iter().all(is_number_like_for_add),
        _ => false,
    }
}

fn evaluate_arithmetic_binary(
    left_result: InferredExpression,
    right_result: InferredExpression,
    left_span: Option<SyntaxTextSpan>,
    right_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(left_type) = inferred_type(&left_result) else {
        return InferredExpression::Unknown;
    };
    let Some(right_type) = inferred_type(&right_result) else {
        return InferredExpression::Unknown;
    };

    if matches!(left_type, Type::Unknown) || matches!(right_type, Type::Unknown) {
        return InferredExpression::Unknown;
    }

    if matches!(left_type, Type::Any) || matches!(right_type, Type::Any) {
        return InferredExpression::Known(Type::Any);
    }

    let left_valid = is_number_like_for_arithmetic(left_type);
    let right_valid = is_number_like_for_arithmetic(right_type);

    match (left_valid, right_valid) {
        (true, true) => InferredExpression::Known(Type::Number),
        (false, true) => {
            let file_name = ctx.file_name.clone();
            push_diagnostic(ctx, Diagnostic::ts2362(file_name), left_span);
            InferredExpression::Unknown
        }
        (true, false) => {
            let file_name = ctx.file_name.clone();
            push_diagnostic(ctx, Diagnostic::ts2363(file_name), right_span);
            InferredExpression::Unknown
        }
        (false, false) => {
            let file_name = ctx.file_name.clone();
            push_diagnostic(ctx, Diagnostic::ts2362(file_name.clone()), left_span);
            push_diagnostic(ctx, Diagnostic::ts2363(file_name), right_span);
            InferredExpression::Unknown
        }
    }
}

fn evaluate_comparison_binary(
    left_result: InferredExpression,
    right_result: InferredExpression,
    operator_text: &'static str,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(left_type) = inferred_type(&left_result) else {
        return InferredExpression::Unknown;
    };
    let Some(right_type) = inferred_type(&right_result) else {
        return InferredExpression::Unknown;
    };

    if matches!(left_type, Type::Unknown) || matches!(right_type, Type::Unknown) {
        return InferredExpression::Unknown;
    }

    if matches!(left_type, Type::Any) || matches!(right_type, Type::Any) {
        return InferredExpression::Known(Type::Boolean);
    }

    if is_comparison_operand_valid(left_type) && is_comparison_operand_valid(right_type) {
        let left_base = left_type.base_primitive();
        let right_base = right_type.base_primitive();

        match (left_base.as_ref(), right_base.as_ref()) {
            (Some(Type::Number), Some(Type::Number)) | (Some(Type::String), Some(Type::String)) => {
                InferredExpression::Known(Type::Boolean)
            }
            _ => {
                let file_name = ctx.file_name.clone();
                push_diagnostic(
                    ctx,
                    Diagnostic::ts2365(
                        operator_text,
                        &operand_display_name(&left_type),
                        &operand_display_name(&right_type),
                        file_name,
                    ),
                    fallback_span,
                );
                InferredExpression::Unknown
            }
        }
    } else {
        let file_name = ctx.file_name.clone();
        push_diagnostic(
            ctx,
            Diagnostic::ts2365(
                operator_text,
                &operand_display_name(&left_type),
                &operand_display_name(&right_type),
                file_name,
            ),
            fallback_span,
        );
        InferredExpression::Unknown
    }
}

fn evaluate_equality_binary(
    left_result: InferredExpression,
    right_result: InferredExpression,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(left_type) = inferred_type(&left_result) else {
        return InferredExpression::Unknown;
    };
    let Some(right_type) = inferred_type(&right_result) else {
        return InferredExpression::Unknown;
    };

    if matches!(left_type, Type::Unknown) || matches!(right_type, Type::Unknown) {
        return InferredExpression::Unknown;
    }

    if matches!(left_type, Type::Any) || matches!(right_type, Type::Any) {
        return InferredExpression::Known(Type::Boolean);
    }

    if !types_overlap_for_equality(left_type, right_type) {
        let file_name = ctx.file_name.clone();
        push_diagnostic(
            ctx,
            Diagnostic::ts2367(
                &operand_display_name(left_type),
                &operand_display_name(right_type),
                file_name,
            ),
            fallback_span,
        );
    }

    InferredExpression::Known(Type::Boolean)
}

fn inferred_type(result: &InferredExpression) -> Option<&Type> {
    match result {
        InferredExpression::Known(ty) => Some(ty),
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => None,
    }
}

fn is_known_non_unknown(result: &InferredExpression) -> bool {
    matches!(result, InferredExpression::Known(ty) if *ty != Type::Unknown)
}

fn is_number_like_for_arithmetic(ty: &Type) -> bool {
    matches!(ty.base_primitive(), Some(Type::Number))
}

fn is_comparison_operand_valid(ty: &Type) -> bool {
    matches!(ty.base_primitive(), Some(Type::Number) | Some(Type::String))
}

fn types_overlap_for_equality(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Union(left_union), Type::Union(right_union)) => {
            left_union.types().iter().any(|left_ty| {
                right_union
                    .types()
                    .iter()
                    .any(|right_ty| types_overlap_for_equality(left_ty, right_ty))
            })
        }
        (Type::Union(left_union), right_ty) => left_union
            .types()
            .iter()
            .any(|left_ty| types_overlap_for_equality(left_ty, right_ty)),
        (left_ty, Type::Union(right_union)) => right_union
            .types()
            .iter()
            .any(|right_ty| types_overlap_for_equality(left_ty, right_ty)),
        (Type::StringLiteral(left_value), Type::StringLiteral(right_value)) => {
            left_value == right_value
        }
        (Type::NumberLiteral(left_value), Type::NumberLiteral(right_value)) => {
            left_value == right_value
        }
        (Type::BooleanLiteral(left_value), Type::BooleanLiteral(right_value)) => {
            left_value == right_value
        }
        (Type::StringLiteral(_), Type::String) | (Type::String, Type::StringLiteral(_)) => true,
        (Type::NumberLiteral(_), Type::Number) | (Type::Number, Type::NumberLiteral(_)) => true,
        (Type::BooleanLiteral(_), Type::Boolean) | (Type::Boolean, Type::BooleanLiteral(_)) => true,
        (Type::String, Type::String)
        | (Type::Number, Type::Number)
        | (Type::Boolean, Type::Boolean)
        | (Type::Undefined, Type::Undefined)
        | (Type::Object(_), Type::Object(_))
        | (Type::Function(_), Type::Function(_)) => true,
        _ => false,
    }
}

fn push_diagnostic(ctx: &mut CheckerContext, diagnostic: Diagnostic, span: Option<SyntaxTextSpan>) {
    let diagnostic = match span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };

    ctx.push(diagnostic);
}

fn binary_operator_text(operator: ParsedBinaryOperator) -> &'static str {
    match operator {
        ParsedBinaryOperator::LessThan => "<",
        ParsedBinaryOperator::LessThanEquals => "<=",
        ParsedBinaryOperator::GreaterThan => ">",
        ParsedBinaryOperator::GreaterThanEquals => ">=",
        ParsedBinaryOperator::Add => "+",
        ParsedBinaryOperator::Subtract => "-",
        ParsedBinaryOperator::Multiply => "*",
        ParsedBinaryOperator::Divide => "/",
        ParsedBinaryOperator::Remainder => "%",
        ParsedBinaryOperator::Exponential => "**",
        ParsedBinaryOperator::ShiftLeft => "<<",
        ParsedBinaryOperator::ShiftRight => ">>",
        ParsedBinaryOperator::ShiftRightZeroFill => ">>>",
        ParsedBinaryOperator::BitwiseAnd => "&",
        ParsedBinaryOperator::BitwiseOR => "|",
        ParsedBinaryOperator::BitwiseXOR => "^",
        ParsedBinaryOperator::StrictEquals
        | ParsedBinaryOperator::StrictNotEquals
        | ParsedBinaryOperator::Equals
        | ParsedBinaryOperator::NotEquals => "==",
        ParsedBinaryOperator::In => "in",
    }
}
