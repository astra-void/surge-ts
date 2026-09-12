use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason};

use crate::context::CheckerContext;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};
use super::guards::*;
use super::{ReferenceGuard, reference_path};

/// The key a narrowable element access is remembered under: `xs[0]`,
/// `db.posts[nextIndex]`. tsc narrows an element access whose key is a
/// literal or a `const` identifier; surge cannot see constness, so any
/// identifier key qualifies — an assignment to the key inside the guarded
/// block would then read a stale narrowing, which is rare and only under-reports.
pub(crate) fn element_reference_key(
    object: &ParsedExpression,
    index: &ParsedExpression,
) -> Option<String> {
    let (base, path) = reference_path(object)?;
    element_reference_key_named(&base, &path, index)
}

pub(crate) fn element_reference_key_named(
    base: &str,
    path: &[String],
    index: &ParsedExpression,
) -> Option<String> {
    let key = match index {
        ParsedExpression::NumberLiteral(value) => value.as_str(),
        ParsedExpression::Identifier { name, .. } => name.as_str(),
        _ => return None,
    };
    let mut rendered = String::with_capacity(base.len() + 8);
    rendered.push_str(base);
    for segment in path {
        rendered.push('.');
        rendered.push_str(segment);
    }
    rendered.push('[');
    rendered.push_str(key);
    rendered.push(']');
    Some(rendered)
}

pub(super) fn element_access_parts(expression: &ParsedExpression) -> Option<String> {
    match expression {
        ParsedExpression::ElementAccess { object, index, .. }
        | ParsedExpression::OptionalIndexAccess { object, index, .. } => {
            element_reference_key(object, index)
        }
        ParsedExpression::IndexAccess {
            object_name, index, ..
        } => element_reference_key_named(object_name, &[], index),
        _ => None,
    }
}

/// Guards on element accesses (`if (xs[0])`, `xs[i] !== undefined`) narrow the
/// access itself, which no binding's type can express: an array has one element
/// type for every index. The narrowed read is remembered under the access's
/// rendered key and consulted by the element-access inference paths.
pub(super) fn narrow_element_reference_guards_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) {
    let narrowed = {
        let visible = scopes.visible_symbols();
        narrowed_element_references(condition, branch_is_true, visible, ctx)
    };
    for (key, narrowed, declared) in narrowed {
        let _ = scopes.insert_current_narrowed(
            key,
            SymbolInfo {
                ty: narrowed,
                kind: crate::symbols::SymbolKind::Var,
                function_signature: None,
            },
            declared,
        );
    }
}

/// Symbol-table counterpart of [`narrow_element_reference_guards_in_scope`],
/// for the operand and branch positions (`xs[i] && xs[i].x`, `xs[i] ? … : …`)
/// that narrow a table rather than the scope stack.
pub(crate) fn narrow_element_reference_guards_symbol_table(
    condition: &ParsedExpression,
    branch_is_true: bool,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<SymbolTable> {
    let narrowed = narrowed_element_references(condition, branch_is_true, symbols, ctx);
    if narrowed.is_empty() {
        return None;
    }
    let mut table = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    for (key, narrowed, declared) in narrowed {
        table.insert_narrowed(
            key,
            SymbolInfo {
                ty: narrowed,
                kind: crate::symbols::SymbolKind::Var,
                function_signature: None,
            },
            declared,
        );
    }
    Some(table)
}

/// `(key, narrowed, declared)` for every element-access guard `condition`
/// proves in the branch.
pub(super) fn narrowed_element_references(
    condition: &ParsedExpression,
    branch_is_true: bool,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Vec<(String, Type, Type)> {
    let mut guards = Vec::new();
    collect_element_reference_guards(condition, branch_is_true, &mut guards);
    let mut narrowed = Vec::new();
    for (access, guard) in guards {
        let Some(key) = element_access_parts(access) else {
            continue;
        };
        let declared = match crate::infer::infer_expression(access, symbols, ctx) {
            crate::infer::InferredExpression::Known(ty) => ty,
            _ => continue,
        };
        if declared.is_unknown() {
            continue;
        }
        let Some((narrowed_ty, _)) = guard.narrow_leaf(&declared, false) else {
            continue;
        };
        if narrowed_ty == declared {
            continue;
        }
        narrowed.push((key, narrowed_ty, declared));
    }
    narrowed
}

pub(super) fn collect_element_reference_guards<'a>(
    condition: &'a ParsedExpression,
    branch_is_true: bool,
    guards: &mut Vec<(&'a ParsedExpression, ReferenceGuard<'a>)>,
) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_element_reference_guards(operand, !branch_is_true, guards),
        ParsedExpression::Logical {
            left,
            operator,
            right,
            ..
        } if matches!(
            (operator, branch_is_true),
            (ParsedLogicalOperator::And, true) | (ParsedLogicalOperator::Or, false)
        ) =>
        {
            collect_element_reference_guards(left, branch_is_true, guards);
            collect_element_reference_guards(right, branch_is_true, guards);
        }
        ParsedExpression::ElementAccess { .. }
        | ParsedExpression::IndexAccess { .. }
        | ParsedExpression::OptionalIndexAccess { .. } => {
            if branch_is_true {
                guards.push((condition, ReferenceGuard::Truthy));
            }
        }
        _ => {
            // `typeof args[0] === 'string'`. The identifier path narrows a
            // binding's own symbol, which an element access has none of, so the
            // tag test has to reach the same per-access record the truthiness and
            // nullish guards use.
            if let Some((subject, tag, eq)) = super::guards::parse_typeof_condition(condition)
                && matches!(
                    subject,
                    ParsedExpression::ElementAccess { .. }
                        | ParsedExpression::IndexAccess { .. }
                        | ParsedExpression::OptionalIndexAccess { .. }
                )
            {
                guards.push((
                    subject,
                    ReferenceGuard::Typeof {
                        tag,
                        keep_matching: branch_is_true == eq,
                    },
                ));
                return;
            }
            if let Some((subject, eq)) = parse_nullish_equality_condition(condition)
                && matches!(
                    subject,
                    ParsedExpression::ElementAccess { .. }
                        | ParsedExpression::IndexAccess { .. }
                        | ParsedExpression::OptionalIndexAccess { .. }
                )
            {
                guards.push((
                    subject,
                    ReferenceGuard::Nullish {
                        keep_matching: branch_is_true == eq,
                    },
                ));
            }
        }
    }
}
