//! Unary, binary, logical, and conditional expression inference.

use super::*;

use surge_ts_syntax::{ParsedBinaryOperator, ParsedExpression, ParsedUnaryOperator};
use surge_ts_types::{Type, union_type};

use crate::context::CheckerContext;
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

pub(crate) fn infer_unary_expression(
    operator: ParsedUnaryOperator,
    operand: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let operand_type = infer_expression(operand, symbols, ctx);

    match operator {
        ParsedUnaryOperator::Not => {
            if is_known_non_unknown(&operand_type) {
                InferredExpression::Known(Type::Boolean)
            } else {
                InferredExpression::Unknown
            }
        }
        ParsedUnaryOperator::Typeof => InferredExpression::Known(Type::String),
        ParsedUnaryOperator::Plus | ParsedUnaryOperator::Minus => match operand_type {
            InferredExpression::Known(Type::Any) => InferredExpression::Known(Type::Number),
            InferredExpression::Known(ty) if matches!(ty.base_primitive(), Some(Type::Number)) => {
                InferredExpression::Known(Type::Number)
            }
            InferredExpression::Known(Type::Unknown)
            | InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown
            | InferredExpression::Known(Type::Undefined)
            | InferredExpression::Known(Type::Void)
            | InferredExpression::Known(Type::String)
            | InferredExpression::Known(Type::Number)
            | InferredExpression::Known(Type::Boolean)
            | InferredExpression::Known(Type::StringLiteral(_))
            | InferredExpression::Known(Type::NumberLiteral(_))
            | InferredExpression::Known(Type::BooleanLiteral(_))
            | InferredExpression::Known(Type::Object(_))
            | InferredExpression::Known(Type::Array(_))
            | InferredExpression::Known(Type::Tuple(_))
            | InferredExpression::Known(Type::Function(_))
            | InferredExpression::Known(Type::Never)
            | InferredExpression::Known(Type::Reference(_))
            | InferredExpression::Known(Type::Union(_)) => InferredExpression::Unknown,
        },
    }
}

pub(crate) fn infer_logical_expression(
    operator: surge_ts_syntax::ParsedLogicalOperator,
    left: &ParsedExpression,
    right: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let left_type = infer_expression(left, symbols, ctx);
    let right_type = infer_expression(right, symbols, ctx);

    match (left_type, right_type) {
        (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
            if left_ty != Type::Unknown && right_ty != Type::Unknown =>
        {
            // `a || b` -> `NonNullable<a> | b`; `a && b` -> `a | b`. See
            // `ops::evaluate_logical_expression`.
            let result = match operator {
                surge_ts_syntax::ParsedLogicalOperator::Or => {
                    union_type(vec![surge_ts_types::remove_nullish(&left_ty), right_ty])
                }
                surge_ts_syntax::ParsedLogicalOperator::And => {
                    union_type(vec![left_ty, right_ty])
                }
            };
            InferredExpression::Known(result)
        }
        _ => InferredExpression::Unknown,
    }
}

pub(crate) fn infer_conditional_expression(
    condition: &ParsedExpression,
    when_true: &ParsedExpression,
    when_false: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let condition_type = infer_expression(condition, symbols, ctx);
    if !is_known_non_unknown(&condition_type) {
        return InferredExpression::Unknown;
    }

    // Narrow a discriminated union for each branch: `x.kind === "a" ? x.a : x.b`
    // sees `x` as the `"a"` member in `when_true` and its complement in
    // `when_false`.
    let true_symbols = crate::checks::function::narrow_condition_symbol_table(
        condition, symbols, true,
    );
    let false_symbols = crate::checks::function::narrow_condition_symbol_table(
        condition, symbols, false,
    );
    let true_type = infer_expression(when_true, true_symbols.as_ref().unwrap_or(symbols), ctx);
    let false_type = infer_expression(when_false, false_symbols.as_ref().unwrap_or(symbols), ctx);

    match (true_type, false_type) {
        (InferredExpression::Known(Type::Any), _) | (_, InferredExpression::Known(Type::Any)) => {
            InferredExpression::Known(Type::Any)
        }
        (InferredExpression::Known(true_ty), InferredExpression::Known(false_ty))
            if true_ty != Type::Unknown && false_ty != Type::Unknown =>
        {
            if true_ty == false_ty {
                InferredExpression::Known(true_ty)
            } else {
                InferredExpression::Known(union_type(vec![true_ty, false_ty]))
            }
        }
        _ => InferredExpression::Unknown,
    }
}

pub(crate) fn infer_binary_expression(
    operator: ParsedBinaryOperator,
    left: &ParsedExpression,
    right: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match operator {
        ParsedBinaryOperator::StrictEquals
        | ParsedBinaryOperator::StrictNotEquals
        | ParsedBinaryOperator::Equals
        | ParsedBinaryOperator::NotEquals
        | ParsedBinaryOperator::LessThan
        | ParsedBinaryOperator::LessThanEquals
        | ParsedBinaryOperator::GreaterThan
        | ParsedBinaryOperator::GreaterThanEquals
        | ParsedBinaryOperator::In
        | ParsedBinaryOperator::Instanceof => InferredExpression::Known(Type::Boolean),
        ParsedBinaryOperator::Add => {
            let left_type = infer_expression(left, symbols, ctx);
            let right_type = infer_expression(right, symbols, ctx);

            match (left_type, right_type) {
                (InferredExpression::Known(Type::Any), _)
                | (_, InferredExpression::Known(Type::Any)) => InferredExpression::Known(Type::Any),
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::String))
                        && matches!(right_ty.base_primitive(), Some(Type::String)) =>
                {
                    InferredExpression::Known(Type::String)
                }
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::String))
                        && matches!(right_ty.base_primitive(), Some(Type::Number)) =>
                {
                    InferredExpression::Known(Type::String)
                }
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::Number))
                        && matches!(right_ty.base_primitive(), Some(Type::String)) =>
                {
                    InferredExpression::Known(Type::String)
                }
                (InferredExpression::Known(left_ty), InferredExpression::Known(right_ty))
                    if matches!(left_ty.base_primitive(), Some(Type::Number))
                        && matches!(right_ty.base_primitive(), Some(Type::Number)) =>
                {
                    InferredExpression::Known(Type::Number)
                }
                _ => InferredExpression::Known(Type::Number),
            }
        }
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
        | ParsedBinaryOperator::BitwiseXOR => InferredExpression::Known(Type::Number),
    }
}
