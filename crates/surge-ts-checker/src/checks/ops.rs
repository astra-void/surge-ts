use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedBinaryOperator, ParsedUnaryOperator, TextSpan as SyntaxTextSpan};
use surge_ts_types::{Type, union_type};

use crate::checks::expr::{operand_display_name, widen_type};
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
            left_span.or(fallback_span),
            operator_span.or(fallback_span),
            right_span.or(fallback_span),
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
            operator,
            left_span.or(fallback_span),
            operator_span,
            right_span.or(fallback_span),
            fallback_span,
            ctx,
        ),
        ParsedBinaryOperator::LessThan
        | ParsedBinaryOperator::LessThanEquals
        | ParsedBinaryOperator::GreaterThan
        | ParsedBinaryOperator::GreaterThanEquals => evaluate_comparison_binary(
            left_result,
            right_result,
            binary_operator_text(operator),
            left_span.or(fallback_span),
            right_span.or(fallback_span),
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
        // `"prop" in obj` / `x instanceof Ctor` are boolean type-guard tests. The
        // operands are already evaluated by the caller; the result is `boolean`.
        ParsedBinaryOperator::In => InferredExpression::Known(Type::Boolean),
        ParsedBinaryOperator::Instanceof => {
            crate::checks::expr::check_instanceof_left_operand(
                &left_result,
                left_span.or(fallback_span),
                ctx,
            );
            crate::checks::expr::check_instanceof_right_operand(
                &right_result,
                right_span.or(fallback_span),
                ctx,
            );
            InferredExpression::Known(Type::Boolean)
        }
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
    if left_ty.is_unknown() || right_ty.is_unknown() {
        return InferredExpression::Unknown;
    }

    // A logical expression yields one of its operand *values*, not `boolean`:
    // `a || b` is `truthy(a) | b` (the left's falsy members are gone when it does
    // not fall through), and `a && b` is `falsy(a) | b` (`a`'s falsy part when it
    // stops the chain, otherwise `b`). `??` has its own handler. Modelling the
    // operand union avoids false assignability errors like
    // `string | undefined || "x"` being treated as `boolean`.
    let result = match operator {
        surge_ts_syntax::ParsedLogicalOperator::Or => surge_ts_types::union_type(vec![
            crate::infer::truthy_part(left_ty),
            right_ty.clone(),
        ]),
        surge_ts_syntax::ParsedLogicalOperator::And => surge_ts_types::union_type(vec![
            crate::infer::falsy_part(left_ty),
            right_ty.clone(),
        ]),
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

    if true_type.is_unknown() || false_type.is_unknown() {
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
) -> InferredExpression {
    match operator {
        ParsedUnaryOperator::Not => match inferred_type(&operand_result) {
            Some(operand_type) if !operand_type.is_unknown() => InferredExpression::Known(
                crate::infer::expression::logical_not_result_type(&operand_type),
            ),
            _ => InferredExpression::Unknown,
        },
        ParsedUnaryOperator::Typeof => {
            InferredExpression::Known(crate::infer::expression::typeof_result_type())
        }
        ParsedUnaryOperator::Delete => InferredExpression::Known(Type::Boolean),
        ParsedUnaryOperator::Void => InferredExpression::Known(Type::Undefined),
        // Unary `+`/`-` coerce: tsc accepts any operand and types the result
        // `number` (`bigint` for a bigint operand). TS2356 is the `++`/`--`
        // operand rule, not this one — reporting it here made `+data` on a
        // contextually-typed `string` parameter a false positive.
        ParsedUnaryOperator::Plus | ParsedUnaryOperator::Minus | ParsedUnaryOperator::BitwiseNot => {
            let Some(operand_type) = inferred_type(&operand_result) else {
                return InferredExpression::Unknown;
            };

            if operand_type.is_unknown() {
                return InferredExpression::Unknown;
            }

            if matches!(operand_type, Type::Any) {
                return InferredExpression::Known(Type::Any);
            }

            // Unary `+` is always `number`; it rejects a bigint operand instead.
            if !matches!(operator, ParsedUnaryOperator::Plus)
                && matches!(operand_type.base_primitive(), Some(Type::BigInt))
            {
                return InferredExpression::Known(Type::BigInt);
            }

            InferredExpression::Known(Type::Number)
        }
    }
}

fn evaluate_add_binary(
    left_result: InferredExpression,
    right_result: InferredExpression,
    left_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    right_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(left_type) = inferred_type(&left_result) else {
        return InferredExpression::Unknown;
    };
    let Some(right_type) = inferred_type(&right_result) else {
        return InferredExpression::Unknown;
    };

    // The operands may be nominal references (`core.output<T>` around the
    // awaited/inferred type). Every classification below matches on structure,
    // so a reference that resolves to `any`/`string`/`number` has to be peeled
    // first or it falls through to the report.
    let left_type = left_type.peeled();
    let right_type = right_type.peeled();

    if is_unmodelled(&left_type) || is_unmodelled(&right_type) {
        return InferredExpression::Unknown;
    }

    // tsc's `+` arm of `checkBinaryLikeExpression`: the result kind is decided
    // first, and only an operand pair with no result kind is an error. A
    // symbol cannot join even a well-typed `+` (`checkForDisallowedESSymbolOperand`).
    let both = |target: &Type| {
        is_strictly_assignable_to(&left_type, target) && is_strictly_assignable_to(&right_type, target)
    };
    let result = if both(&Type::Number) {
        Some(Type::Number)
    } else if both(&Type::BigInt) {
        Some(Type::BigInt)
    } else if is_strictly_assignable_to(&left_type, &Type::String)
        || is_strictly_assignable_to(&right_type, &Type::String)
    {
        Some(Type::String)
    } else if matches!(left_type, Type::Any) || matches!(right_type, Type::Any) {
        Some(Type::Any)
    } else {
        None
    };
    if let Some(result) = result {
        report_symbol_operand("+", &left_type, &right_type, left_span, right_span, ctx);
        return InferredExpression::Known(result);
    }

    // `getBaseTypesIfUnrelated`: the operands keep their literal names when
    // their base types are both ones `+` could plausibly have meant.
    let close_enough = |ty: &Type| {
        *ty == Type::GenuineUnknown
            || [Type::Number, Type::BigInt, Type::String]
            .iter()
            .any(|target| surge_ts_types::is_assignable_to(ty, target))
    };
    let (left_base, right_base) = (widen_type(&left_type), widen_type(&right_type));
    let (left_name, right_name) = if close_enough(&left_base) && close_enough(&right_base) {
        (left_type.name(), right_type.name())
    } else {
        (left_base.name(), right_base.name())
    };
    let file_name = ctx.file_name.clone();
    push_diagnostic(
        ctx,
        Diagnostic::ts2365("+", &left_name, &right_name, file_name),
        fallback_span,
    );
    InferredExpression::Unknown
}

/// `isTypeAssignableToKindEx(ty, kind, strict = true)`: the operand relates to
/// the kind's primitive, where `any`, `unknown`, `void` and `undefined` do not
/// count on their own.
fn is_strictly_assignable_to(ty: &Type, target: &Type) -> bool {
    !matches!(ty, Type::Any | Type::Void | Type::Undefined)
        && !ty.is_unknown()
        && surge_ts_types::is_assignable_to(ty, target)
}

/// TS2469 on the first operand that may be a symbol. Returns whether one was.
fn report_symbol_operand(
    operator_text: &str,
    left_type: &Type,
    right_type: &Type,
    left_span: Option<SyntaxTextSpan>,
    right_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    fn maybe_symbol(ty: &Type) -> bool {
        match ty {
            Type::Symbol => true,
            Type::Union(union) => union.types().iter().any(maybe_symbol),
            _ => false,
        }
    }
    let span = if maybe_symbol(left_type) {
        left_span
    } else if maybe_symbol(right_type) {
        right_span
    } else {
        return false;
    };
    let file_name = ctx.file_name.clone();
    push_diagnostic(ctx, Diagnostic::ts2469(operator_text, file_name), span);
    true
}

/// tsc's arithmetic and bitwise arm of `checkBinaryLikeExpression`: two
/// boolean operands of `&`/`|`/`^` are TS2447 on the operator; otherwise each
/// operand must be `any`, number-like or bigint-like (TS2362/TS2363), and
/// mixing a bigint with a non-bigint — or `>>>` on bigints — is TS2365 on the
/// whole expression.
#[allow(clippy::too_many_arguments)]
fn evaluate_arithmetic_binary(
    left_result: InferredExpression,
    right_result: InferredExpression,
    operator: ParsedBinaryOperator,
    left_span: Option<SyntaxTextSpan>,
    operator_span: Option<SyntaxTextSpan>,
    right_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(left_type) = inferred_type(&left_result) else {
        return InferredExpression::Unknown;
    };
    let Some(right_type) = inferred_type(&right_result) else {
        return InferredExpression::Unknown;
    };

    if is_unmodelled(&left_type) || is_unmodelled(&right_type) {
        return InferredExpression::Unknown;
    }

    if let Some(suggested) = suggested_boolean_operator(operator)
        && is_boolean_like(left_type)
        && is_boolean_like(right_type)
    {
        let file_name = ctx.file_name.clone();
        push_diagnostic(
            ctx,
            Diagnostic::ts2447(binary_operator_text(operator), suggested, file_name),
            operator_span.or(fallback_span),
        );
        return InferredExpression::Known(Type::Number);
    }

    let left_valid = is_valid_arithmetic_operand(left_type);
    let right_valid = is_valid_arithmetic_operand(right_type);
    if !left_valid {
        let file_name = ctx.file_name.clone();
        push_diagnostic(ctx, Diagnostic::ts2362(file_name), left_span);
    }
    if !right_valid {
        let file_name = ctx.file_name.clone();
        push_diagnostic(ctx, Diagnostic::ts2363(file_name), right_span);
    }

    // Without a bigint in play the result is `number`, `any` operands included.
    if !maybe_bigint_like(left_type) && !maybe_bigint_like(right_type) {
        return if left_valid && right_valid {
            InferredExpression::Known(Type::Number)
        } else {
            InferredExpression::Unknown
        };
    }
    if is_bigint_like(left_type) && is_bigint_like(right_type) {
        if matches!(operator, ParsedBinaryOperator::ShiftRightZeroFill) {
            report_operator_error(operator, left_type, right_type, fallback_span, ctx);
        }
        return InferredExpression::Known(Type::BigInt);
    }
    report_operator_error(operator, left_type, right_type, fallback_span, ctx);
    InferredExpression::Unknown
}

fn report_operator_error(
    operator: ParsedBinaryOperator,
    left_type: &Type,
    right_type: &Type,
    span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    let file_name = ctx.file_name.clone();
    push_diagnostic(
        ctx,
        Diagnostic::ts2365(
            binary_operator_text(operator),
            &operand_display_name(left_type),
            &operand_display_name(right_type),
            file_name,
        ),
        span,
    );
}

fn suggested_boolean_operator(operator: ParsedBinaryOperator) -> Option<&'static str> {
    match operator {
        ParsedBinaryOperator::BitwiseAnd => Some("&&"),
        ParsedBinaryOperator::BitwiseOR => Some("||"),
        ParsedBinaryOperator::BitwiseXOR => Some("!=="),
        _ => None,
    }
}

fn is_boolean_like(ty: &Type) -> bool {
    match ty {
        Type::Boolean | Type::BooleanLiteral(_) => true,
        Type::Union(union) => union.types().iter().all(is_boolean_like),
        _ => false,
    }
}

/// `isTypeAssignableToKind(t, BigIntLike)`, which `any` satisfies.
fn is_bigint_like(ty: &Type) -> bool {
    match ty {
        Type::BigInt | Type::Any => true,
        Type::Union(union) => union.types().iter().all(is_bigint_like),
        _ => false,
    }
}

fn maybe_bigint_like(ty: &Type) -> bool {
    match ty {
        Type::BigInt => true,
        Type::Union(union) => union.types().iter().any(maybe_bigint_like),
        _ => false,
    }
}

/// `checkArithmeticOperandType`: `any`, number-like (numeric enums included)
/// or bigint-like.
fn is_valid_arithmetic_operand(ty: &Type) -> bool {
    match ty {
        Type::Any | Type::BigInt => true,
        Type::Union(union) => union.types().iter().all(is_valid_arithmetic_operand),
        other => is_number_like_for_arithmetic(other),
    }
}

fn evaluate_comparison_binary(
    left_result: InferredExpression,
    right_result: InferredExpression,
    operator_text: &'static str,
    left_span: Option<SyntaxTextSpan>,
    right_span: Option<SyntaxTextSpan>,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    let Some(left_type) = inferred_type(&left_result) else {
        return InferredExpression::Unknown;
    };
    let Some(right_type) = inferred_type(&right_result) else {
        return InferredExpression::Unknown;
    };

    if is_unmodelled(&left_type) || is_unmodelled(&right_type) {
        return InferredExpression::Unknown;
    }

    if report_symbol_operand(operator_text, left_type, right_type, left_span, right_span, ctx) {
        return InferredExpression::Known(Type::Boolean);
    }

    if matches!(left_type, Type::Any) || matches!(right_type, Type::Any) {
        return InferredExpression::Known(Type::Boolean);
    }

    // `never` is comparable to every type, so tsc reports no overlap error on a
    // comparison in an already-dead branch.
    if matches!(left_type, Type::Never) || matches!(right_type, Type::Never) {
        return InferredExpression::Known(Type::Boolean);
    }

    // tsc's `checkBinaryLikeExpression` for `<`/`>`/`<=`/`>=`: both operands
    // numeric (`number | bigint`), or neither numeric and comparable to each
    // other, with literals compared by their base type.
    let left_numeric = is_numeric_comparison_operand(left_type);
    let right_numeric = is_numeric_comparison_operand(right_type);
    let comparable = || {
        let left_base = widen_literal_for_comparison(left_type);
        let right_base = widen_literal_for_comparison(right_type);
        surge_ts_types::is_assignable_to(&left_base, &right_base)
            || surge_ts_types::is_assignable_to(&right_base, &left_base)
    };
    if (left_numeric && right_numeric) || (!left_numeric && !right_numeric && comparable()) {
        return InferredExpression::Known(Type::Boolean);
    }
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

    if left_type.is_unknown() || right_type.is_unknown() {
        return InferredExpression::Unknown;
    }

    if matches!(left_type, Type::Any) || matches!(right_type, Type::Any) {
        return InferredExpression::Known(Type::Boolean);
    }

    // `never` is comparable to every type, so tsc reports no overlap error on a
    // comparison in an already-dead branch.
    if matches!(left_type, Type::Never) || matches!(right_type, Type::Never) {
        return InferredExpression::Known(Type::Boolean);
    }

    // tsc's `isTypeEqualityComparableTo` short-circuits when the *other* operand
    // is nullable, so `x === undefined` / `x === null` never reports TS2367
    // whatever `x` is. The test is on the operand as a whole, not on individual
    // union constituents, so it stays here rather than inside the overlap walk.
    if matches!(left_type, Type::Undefined | Type::Null)
        || matches!(right_type, Type::Undefined | Type::Null)
    {
        return InferredExpression::Known(Type::Boolean);
    }

    if !types_overlap_for_equality(left_type, right_type) {
        let file_name = ctx.file_name.clone();
        let (left_name, right_name) = equality_operand_display_names(left_type, right_type);
        push_diagnostic(
            ctx,
            Diagnostic::ts2367(&left_name, &right_name, file_name),
            fallback_span,
        );
    }

    InferredExpression::Known(Type::Boolean)
}

/// Operand names for TS2367, mirroring tsc's `getBaseTypesIfUnrelated`: the
/// widened operands are reported only when widening does not make them
/// comparable. `1 === "string"` reads `'number'` and `'string'`, while
/// `"a" === "b"` and a literal-union subject keep their literal names.
pub(crate) fn equality_operand_display_names(left: &Type, right: &Type) -> (String, String) {
    let left_base = widen_type(left);
    let right_base = widen_type(right);
    if types_overlap_for_equality(&left_base, &right_base) {
        return (left.name(), right.name());
    }
    (left_base.name(), right_base.name())
}

pub(crate) fn inferred_type(result: &InferredExpression) -> Option<&Type> {
    match result {
        InferredExpression::Known(ty) => Some(ty),
        InferredExpression::UnresolvedIdentifier { .. }
        | InferredExpression::MissingProperty { .. }
        | InferredExpression::Unknown => None,
    }
}

fn is_known_non_unknown(result: &InferredExpression) -> bool {
    matches!(result, InferredExpression::Known(ty) if !ty.is_unknown())
}

fn is_number_like_for_arithmetic(ty: &Type) -> bool {
    matches!(ty.base_primitive(), Some(Type::Number))
}

fn is_numeric_comparison_operand(ty: &Type) -> bool {
    match ty {
        Type::Union(union) => union.types().iter().all(is_numeric_comparison_operand),
        other => matches!(other.base_primitive(), Some(Type::Number) | Some(Type::BigInt)),
    }
}

fn widen_literal_for_comparison(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        Type::Union(union) => {
            surge_ts_types::union_type(union.types().iter().map(widen_literal_for_comparison).collect())
        }
        other => other.clone(),
    }
}

pub(crate) fn types_overlap_for_equality(left: &Type, right: &Type) -> bool {
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
        // `any`, `unknown` and `never` are comparable to everything, so a
        // degraded operand — or a union that merely *contains* one — must never
        // reach the disjointness verdict below.
        (Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) | Type::Never, _)
        | (_, Type::Any | Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) | Type::Never) => true,
        (Type::StringLiteral(left_value), Type::StringLiteral(right_value)) => {
            left_value == right_value
        }
        (Type::NumberLiteral(left_value), Type::NumberLiteral(right_value)) => {
            left_value == right_value
        }
        (Type::BooleanLiteral(left_value), Type::BooleanLiteral(right_value)) => {
            left_value == right_value
        }
        (Type::Array(left_element), Type::Array(right_element)) => {
            types_overlap_for_equality(left_element, right_element)
        }
        (Type::Tuple(left_items), Type::Tuple(right_items)) => {
            left_items.len() == right_items.len()
                && left_items
                    .iter()
                    .zip(right_items.iter())
                    .all(|(left_item, right_item)| {
                        types_overlap_for_equality(left_item, right_item)
                    })
        }
        (Type::Array(element), Type::Tuple(items)) | (Type::Tuple(items), Type::Array(element)) => {
            items
                .iter()
                .all(|item| types_overlap_for_equality(item, element))
        }
        // Every non-nullish value is assignable to `{}`, so an empty object type
        // is comparable to anything.
        (Type::Object(object), _) | (_, Type::Object(object)) if is_empty_object_type(object) => {
            true
        }
        // Everything else is disjoint only when *both* operands land in a kind
        // whose inhabitants are known: comparing two different known kinds has
        // no overlap, and anything unclassified (references, type parameters,
        // …) is assumed comparable rather than reported, matching tsc, which
        // issues TS2367 only for provably disjoint operands.
        _ => match (equality_kind(left), equality_kind(right)) {
            (Some(left_kind), Some(right_kind)) => left_kind == right_kind,
            _ => true,
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EqualityKind {
    String,
    Number,
    Boolean,
    BigInt,
    Symbol,
    Undefined,
    Void,
    Object,
    Function,
    Array,
    Tuple,
}

fn equality_kind(ty: &Type) -> Option<EqualityKind> {
    match ty {
        Type::String | Type::StringLiteral(_) => Some(EqualityKind::String),
        Type::Number | Type::NumberLiteral(_) => Some(EqualityKind::Number),
        Type::Boolean | Type::BooleanLiteral(_) => Some(EqualityKind::Boolean),
        Type::BigInt => Some(EqualityKind::BigInt),
        Type::Symbol => Some(EqualityKind::Symbol),
        Type::Undefined | Type::Null => Some(EqualityKind::Undefined),
        Type::Void => Some(EqualityKind::Void),
        Type::Object(_) => Some(EqualityKind::Object),
        Type::Function(_) => Some(EqualityKind::Function),
        Type::Array(_) => Some(EqualityKind::Array),
        Type::Tuple(_) => Some(EqualityKind::Tuple),
        _ => None,
    }
}

fn is_empty_object_type(object: &surge_ts_types::ObjectType) -> bool {
    object.properties.is_empty()
        && object.string_index_type.is_none()
        && object.call_signature.is_none()
        && object.construct_signature.is_none()
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
        ParsedBinaryOperator::Instanceof => "instanceof",
    }
}

/// A type surge could not model, as opposed to the written `unknown`, which an
/// operator judges like any other operand (under `strictNullChecks` it is
/// reported before it gets here).
fn is_unmodelled(ty: &Type) -> bool {
    ty.is_unknown() && *ty != Type::GenuineUnknown
}
