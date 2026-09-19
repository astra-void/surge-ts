//! Per-guard-kind leaf narrowing: recognizing a guard condition, narrowing a
//! union at the type level, and building a narrowed symbol table for it. The
//! orchestration that applies these across scopes and composes them with truthy
//! narrowing lives in the parent module.

use surge_ts_syntax::ParsedExpression;

mod arrayness;
mod discriminant;
mod instanceof;
mod literal_equality;
mod nullish;
mod predicate;
mod property_presence;
mod typeof_guard;

pub(super) use arrayness::*;
pub(super) use discriminant::*;
pub(super) use instanceof::*;
pub(super) use literal_equality::*;
pub(crate) use nullish::*;
pub(super) use predicate::*;
pub(super) use property_presence::*;
pub(super) use typeof_guard::*;
pub(crate) use typeof_guard::typeof_tags_of;

/// The identifier a single type guard tests, if the guard is one we model over a
/// bare identifier (`x instanceof C`, `typeof x === "s"`, `Array.isArray(x)`,
/// `ArrayBuffer.isView(x)`).
pub(super) fn guard_operand_identifier(condition: &ParsedExpression) -> Option<&str> {
    let operand = if let Some((operand, _)) = parse_instanceof_condition(condition) {
        operand
    } else if let Some((operand, _, _)) = parse_typeof_condition(condition) {
        operand
    } else if let Some(operand) = parse_array_isarray_condition(condition) {
        operand
    } else if let Some(operand) = parse_arraybuffer_isview_condition(condition) {
        operand
    } else if let Some((operand, _)) = parse_in_condition(condition) {
        operand
    } else {
        return None;
    };
    match operand {
        ParsedExpression::Identifier { name, .. } => Some(name.as_str()),
        _ => None,
    }
}
