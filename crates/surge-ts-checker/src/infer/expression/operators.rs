//! Unary, binary, logical, and conditional expression inference.

use super::*;

use surge_ts_syntax::{ParsedBinaryOperator, ParsedExpression, ParsedUnaryOperator};
use surge_ts_types::{Type, union_type};

use crate::context::CheckerContext;
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

/// tsc's `!` result (`checkPrefixUnaryExpression`): `false` for an operand
/// that is always truthy, `true` for one always falsy, `boolean` otherwise.
pub(crate) fn logical_not_result_type(operand: &Type) -> Type {
    if matches!(operand.peeled(), Type::Never) {
        return Type::Boolean;
    }
    match crate::checks::function::type_truthiness(operand) {
        Some(truthy) => Type::BooleanLiteral(!truthy),
        None => Type::Boolean,
    }
}

/// tsc's `typeofType`: the union of every `typeof` result, in sorted order.
pub(crate) fn typeof_result_type() -> Type {
    union_type(
        [
            "bigint",
            "boolean",
            "function",
            "number",
            "object",
            "string",
            "symbol",
            "undefined",
        ]
        .into_iter()
        .map(|name| Type::StringLiteral(name.to_string()))
        .collect(),
    )
}

pub(crate) fn infer_unary_expression(
    operator: ParsedUnaryOperator,
    operand: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let operand_type = infer_expression(operand, symbols, ctx);

    match operator {
        ParsedUnaryOperator::Not => match operand_type {
            InferredExpression::Known(ty) if !ty.is_unknown() => {
                InferredExpression::Known(logical_not_result_type(&ty))
            }
            _ => InferredExpression::Unknown,
        },
        ParsedUnaryOperator::Typeof => InferredExpression::Known(typeof_result_type()),
        ParsedUnaryOperator::Delete => InferredExpression::Known(Type::Boolean),
        // `void` / `delete` / `~`: the operand has already been walked, and the
        // result stays unmodelled rather than guessing `undefined`/`boolean`/`number`.
        ParsedUnaryOperator::Discard => InferredExpression::Unknown,
        ParsedUnaryOperator::Plus | ParsedUnaryOperator::Minus => match operand_type {
            InferredExpression::Known(Type::Any) => InferredExpression::Known(Type::Number),
            InferredExpression::Known(ty) if matches!(ty.base_primitive(), Some(Type::Number)) => {
                InferredExpression::Known(Type::Number)
            }
            InferredExpression::Known(Type::Unknown)
            | InferredExpression::Known(Type::ErrorType)
            | InferredExpression::Known(Type::GenuineUnknown)
            | InferredExpression::Known(Type::TypeParameter(_))
            | InferredExpression::UnresolvedIdentifier { .. }
            | InferredExpression::MissingProperty { .. }
            | InferredExpression::Unknown
            | InferredExpression::Known(Type::Undefined)
            | InferredExpression::Known(Type::Void)
            | InferredExpression::Known(Type::String)
            | InferredExpression::Known(Type::Number)
            | InferredExpression::Known(Type::Boolean)
            | InferredExpression::Known(Type::BigInt)
            | InferredExpression::Known(Type::Symbol)
            | InferredExpression::Known(Type::StringLiteral(_))
            | InferredExpression::Known(Type::NumberLiteral(_))
            | InferredExpression::Known(Type::BooleanLiteral(_))
            | InferredExpression::Known(Type::Object(_))
            | InferredExpression::Known(Type::Array(_))
            | InferredExpression::Known(Type::Tuple(_))
            | InferredExpression::Known(Type::OpenTuple(_))
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
            if !left_ty.is_unknown() && !right_ty.is_unknown() =>
        {
            // `a || b` -> `NonNullable<a> | b`; `a && b` -> `a | b`. See
            // `ops::evaluate_logical_expression`.
            let result = match operator {
                surge_ts_syntax::ParsedLogicalOperator::Or => {
                    union_type(vec![surge_ts_types::remove_nullish(&left_ty), right_ty])
                }
                // `a && b` is `b` when `a` is truthy and `a` otherwise, so
                // only `a`'s falsy part survives (`Box | undefined` contributes
                // `undefined`, `string` contributes `""`).
                surge_ts_syntax::ParsedLogicalOperator::And => {
                    union_type(vec![falsy_part(&left_ty), right_ty])
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
    let true_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, true);
    let true_symbols = crate::checks::function::narrow_predicate_guards_symbol_table(
        condition,
        true_symbols.as_ref().unwrap_or(symbols),
        true,
        ctx,
    )
    .or(true_symbols);
    let false_symbols =
        crate::checks::function::narrow_condition_symbol_table(condition, symbols, false);
    let false_symbols = crate::checks::function::narrow_predicate_guards_symbol_table(
        condition,
        false_symbols.as_ref().unwrap_or(symbols),
        false,
        ctx,
    )
    .or(false_symbols);
    // `typeof xs[0] === 'string' ? xs[0] : …` — an element access has no binding
    // to narrow, so its guarded read is recorded per access the way the
    // statement-level evaluator already does it.
    let true_symbols = crate::checks::function::narrow_element_reference_guards_symbol_table(
        condition,
        true,
        true_symbols.as_ref().unwrap_or(symbols),
        ctx,
    )
    .or(true_symbols);
    let false_symbols = crate::checks::function::narrow_element_reference_guards_symbol_table(
        condition,
        false,
        false_symbols.as_ref().unwrap_or(symbols),
        ctx,
    )
    .or(false_symbols);
    let true_type = infer_expression(when_true, true_symbols.as_ref().unwrap_or(symbols), ctx);
    let false_type = infer_expression(when_false, false_symbols.as_ref().unwrap_or(symbols), ctx);

    match (true_type, false_type) {
        (InferredExpression::Known(Type::Any), _) | (_, InferredExpression::Known(Type::Any)) => {
            InferredExpression::Known(Type::Any)
        }
        (InferredExpression::Known(true_ty), InferredExpression::Known(false_ty))
            if !true_ty.is_unknown() && !false_ty.is_unknown() =>
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
                // One string operand makes `+` a concatenation whatever the
                // other side is, which is what keeps `'data' + String(x)` a
                // string when the other operand is one this pass cannot type.
                // Falling through to the numeric default instead disagreed with
                // the checking pass, and a generic call inferring from such an
                // arrow bound its return to `number`.
                (InferredExpression::Known(left_ty), _)
                    if matches!(left_ty.base_primitive(), Some(Type::String)) =>
                {
                    InferredExpression::Known(Type::String)
                }
                (_, InferredExpression::Known(right_ty))
                    if matches!(right_ty.base_primitive(), Some(Type::String)) =>
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

/// The part of `ty` that is falsy, per tsc's `TypeFacts.Falsy`: a nullish or
/// `false`-able member survives, a primitive contributes its falsy literal, and
/// an object never is. `any` and the sentinels are left whole.
pub(crate) fn falsy_part(ty: &Type) -> Type {
    match ty {
        Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) => ty.clone(),
        Type::Undefined | Type::Void => Type::Undefined,
        Type::Boolean => Type::BooleanLiteral(false),
        Type::BooleanLiteral(false) => ty.clone(),
        Type::String => Type::StringLiteral(String::new()),
        Type::StringLiteral(value) if value.is_empty() => ty.clone(),
        Type::Number => Type::NumberLiteral(surge_ts_types::NumberLiteralType {
            value: "0".to_string(),
        }),
        Type::NumberLiteral(literal) if literal.value == "0" => ty.clone(),
        Type::Union(union) => union_type(union.types().iter().map(falsy_part).collect()),
        Type::Reference(reference) => falsy_part(&reference.resolve()),
        _ => Type::Never,
    }
}
