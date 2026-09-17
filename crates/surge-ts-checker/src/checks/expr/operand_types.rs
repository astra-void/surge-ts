//! Operand rules that reject a type outright, ported from
//! `tsc/internal/checker/checker.go`: the object-literal spread
//! (`isValidSpreadType`) and the left-hand side of `instanceof`
//! (`checkInstanceOfExpression`).

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::TextSpan as SyntaxTextSpan;
use surge_ts_types::Type;

use crate::context::{CheckerContext, convert_span};
use crate::infer::InferredExpression;

/// tsc's `removeDefinitelyFalsyTypes`, which `isValidSpreadType` applies before
/// testing: `{ ...maybeObject }` is legal because the `undefined` half is
/// dropped, while `{ ...undefined }` is not, because dropping it leaves nothing.
fn without_definitely_falsy(ty: &Type) -> Option<Type> {
    if is_definitely_falsy(ty) {
        return None;
    }
    let Type::Union(union) = ty else {
        return Some(ty.clone());
    };
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| !is_definitely_falsy(member))
        .cloned()
        .collect();
    if kept.is_empty() {
        return None;
    }
    Some(surge_ts_types::union_type(kept))
}

fn is_definitely_falsy(ty: &Type) -> bool {
    match ty {
        Type::Undefined | Type::Void | Type::Never => true,
        Type::BooleanLiteral(false) => true,
        Type::StringLiteral(value) => value.is_empty(),
        Type::NumberLiteral(value) => value.value == "0",
        _ => false,
    }
}

/// tsc's `isValidSpreadType`: an object, `object`, `any`, or an instantiable
/// non-primitive; a union only when every surviving constituent qualifies.
fn is_valid_spread_type(ty: &Type) -> bool {
    let Some(ty) = without_definitely_falsy(ty) else {
        return false;
    };
    match &ty {
        Type::Any
        | Type::Object(_)
        | Type::Array(_)
        | Type::Tuple(_)
        | Type::OpenTuple(_)
        | Type::Function(_)
        | Type::TypeParameter(_) => true,
        // A modelling failure is not evidence of a bad spread.
        Type::Unknown => true,
        Type::Reference(reference) => is_valid_spread_type(&reference.resolve()),
        Type::Union(union) => union.types().iter().all(is_valid_spread_type),
        _ => false,
    }
}

/// `{ ...source }` where `source` is not an object type — TS2698. tsc reports it
/// on the spread element, not on the object literal or the argument.
pub(crate) fn check_object_spread_type(
    spread_result: &InferredExpression,
    spread_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    let (InferredExpression::Known(ty), Some(span)) = (spread_result, spread_span) else {
        return;
    };
    if is_valid_spread_type(ty) {
        return;
    }
    let file_name = ctx.file_name.clone();
    ctx.push(Diagnostic::ts2698(file_name).with_span(convert_span(span)));
}

/// tsc's rule is `!isTypeAny(left) && allTypesAssignableToKind(left, Primitive)`.
/// `unknown` is deliberately exempt — tsc notes the related error was already
/// reported — and so is a union with any non-primitive constituent.
fn is_all_primitive(ty: &Type) -> bool {
    match ty {
        Type::String
        | Type::Number
        | Type::Boolean
        | Type::BigInt
        | Type::Symbol
        | Type::StringLiteral(_)
        | Type::NumberLiteral(_)
        | Type::BooleanLiteral(_)
        | Type::Undefined
        | Type::Void => true,
        Type::Reference(reference) => is_all_primitive(&reference.resolve()),
        Type::Union(union) => union.types().iter().all(is_all_primitive),
        _ => false,
    }
}

/// `x instanceof C` where `x` is a primitive — TS2358, reported on the left
/// operand.
pub(crate) fn check_instanceof_left_operand(
    left_result: &InferredExpression,
    left_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    let (InferredExpression::Known(ty), Some(span)) = (left_result, left_span) else {
        return;
    };
    if ty.is_unknown() || !is_all_primitive(ty) {
        return;
    }
    let file_name = ctx.file_name.clone();
    ctx.push(Diagnostic::ts2358(file_name).with_span(convert_span(span)));
}
