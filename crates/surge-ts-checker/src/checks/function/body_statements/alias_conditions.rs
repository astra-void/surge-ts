
use surge_ts_syntax::{
    ParsedExpression,
    ParsedFunctionBodyStatement,
    ParsedUnaryOperator,
};
use surge_ts_types::{Type, TypeCopyReason, union_type, with_type_copy_reason};

use crate::context::CheckerContext;
use crate::flow::FunctionFlowState;
use crate::symbols::{ScopeStack, SymbolInfo};
use super::super::{downgrade_genuine_unknown_in_scope, narrow_discriminant_in_scope};

/// Whether an initializer is plainly a boolean condition — a logical chain, a
/// negation, or a comparison. Deliberately narrow: a `const x = f()` is not
/// recorded, so the aliased-condition clone stays proportional to guard
/// aliases rather than to every `const` in the body.
pub(crate) fn is_condition_shaped(expression: &ParsedExpression) -> bool {
    use surge_ts_syntax::{ParsedBinaryOperator, ParsedUnaryOperator};
    match expression {
        ParsedExpression::Logical { .. } => true,
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            ..
        } => true,
        ParsedExpression::Binary { operator, .. } => matches!(
            operator,
            ParsedBinaryOperator::StrictEquals
                | ParsedBinaryOperator::Equals
                | ParsedBinaryOperator::StrictNotEquals
                | ParsedBinaryOperator::NotEquals
                | ParsedBinaryOperator::Instanceof
        ),
        _ => false,
    }
}

/// The `(source, index)` an array-destructured binding was lowered from
/// (`const [a, b] = xs` lowers each binding to `xs[0]`, `xs[1]`).
pub(super) fn tuple_destructure_source(expression: &ParsedExpression) -> Option<(String, usize)> {
    let ParsedExpression::IndexAccess {
        object_name, index, ..
    } = expression
    else {
        return None;
    };
    let ParsedExpression::NumberLiteral(value) = index.as_ref() else {
        return None;
    };
    Some((object_name.clone(), value.parse::<usize>().ok()?))
}

/// The binding an `if` condition proves truthy (`true`) or falsy (`false`),
/// when the condition is exactly that binding or its negation.
pub(super) fn truthiness_tested_binding(
    condition: &ParsedExpression,
    branch_is_true: bool,
) -> Option<(&str, bool)> {
    match condition {
        ParsedExpression::Identifier { name, .. } => Some((name.as_str(), branch_is_true)),
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => truthiness_tested_binding(operand, !branch_is_true),
        _ => None,
    }
}

/// Applies tsc's destructured-discriminated-union narrowing: `const [error,
/// value] = tuple` over a *union of tuples* binds dependent names, so proving
/// `error` falsy rules out the union members whose first element is not
/// nullish, and `value` is retyped from the survivors.
///
/// Only the source's own union is filtered — each sibling is re-derived from
/// it, never narrowed on its own — so a binding whose element is identical in
/// every surviving member keeps exactly the type it already had.
pub(super) fn narrow_tuple_destructure_siblings(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    flow_state: &FunctionFlowState,
) {
    let Some((tested, holds)) = truthiness_tested_binding(condition, branch_is_true) else {
        return;
    };
    let Some((source, tested_index)) = flow_state.tuple_destructure_binding(tested) else {
        return;
    };
    let source = source.to_string();
    let Some(symbol) = scopes.resolve(&source) else {
        return;
    };
    let Type::Union(union) = symbol.ty.peeled() else {
        return;
    };
    let members: Vec<Type> = union.types().iter().map(Type::peeled).collect();
    if !members.iter().all(|member| matches!(member, Type::Tuple(_))) {
        return;
    }

    let kept: Vec<&Type> = members
        .iter()
        .filter(|member| {
            let Type::Tuple(elements) = member else {
                return false;
            };
            match elements.get(tested_index) {
                // Truthy rules out an element that can only be nullish; falsy
                // rules out one that can never be. Anything else stays: this is
                // a discriminant test, not a general truthiness analysis.
                Some(element) => element_is_nullish(element) != holds,
                None => false,
            }
        })
        .collect();
    if kept.is_empty() || kept.len() == members.len() {
        return;
    }

    for (name, index) in flow_state.tuple_destructure_siblings(&source) {
        let Some(symbol) = scopes.resolve(&name) else {
            continue;
        };
        let declared = symbol.ty.clone();
        let kind = symbol.kind;
        let function_signature = symbol.function_signature.clone();
        let selected: Vec<Type> = kept
            .iter()
            .filter_map(|member| match member {
                Type::Tuple(elements) => elements.get(index).cloned(),
                _ => None,
            })
            .collect();
        if selected.len() != kept.len() {
            continue;
        }
        let narrowed = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
            union_type(selected)
        });
        if narrowed == declared {
            continue;
        }
        let _ = scopes.insert_current_narrowed(
            name,
            SymbolInfo {
                ty: narrowed,
                kind,
                function_signature,
            },
            declared,
        );
    }
}

/// Whether a tuple element can only be `undefined`/`null` — the shape a
/// discriminated result tuple uses for its "absent" slot.
pub(super) fn element_is_nullish(element: &Type) -> bool {
    matches!(element, Type::Undefined | Type::Void | Type::Never)
}

/// Narrows by a condition and, when it named a discriminant alias, by the/// Narrows by a condition and, when it named a discriminant alias, by the
/// rewritten form as well. Both are applied: the written condition narrows the
/// alias binding itself (`if (transformer)` proves the local non-nullish), the
/// rewrite narrows the object it came from (`opts.transformer`).
pub(super) fn narrow_condition_and_aliases_in_scope(
    base: &ParsedExpression,
    rewritten: Option<&ParsedExpression>,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    flow_state: &FunctionFlowState,
    ctx: &mut CheckerContext,
) {
    narrow_discriminant_in_scope(base, scopes, branch_is_true, ctx);
    if let Some(rewritten) = rewritten {
        narrow_discriminant_in_scope(rewritten, scopes, branch_is_true, ctx);
    }
    narrow_tuple_destructure_siblings(base, scopes, branch_is_true, flow_state);
}

/// Whether an initializer is a static property reference over identifiers
/// (`opts.direction`, `node.kind.value`) — the shape a discriminant alias takes.
pub(super) fn is_property_reference(expression: &ParsedExpression) -> bool {
    match expression {
        ParsedExpression::PropertyAccess { object, .. }
        | ParsedExpression::OptionalPropertyAccess { object, .. } => {
            matches!(object.as_ref(), ParsedExpression::Identifier { .. })
                || is_property_reference(object)
        }
        _ => false,
    }
}

/// Substitutes discriminant aliases for the references they were bound to, so
/// `if (direction === "up")` narrows `opts` exactly as `opts.direction === "up"`
/// would. `None` when the condition names no alias.
pub(super) fn rewrite_discriminant_aliases(
    condition: &ParsedExpression,
    flow_state: &FunctionFlowState,
) -> Option<ParsedExpression> {
    match condition {
        ParsedExpression::Identifier { name, .. } => flow_state.discriminant_alias(name).cloned(),
        ParsedExpression::Unary {
            operator,
            operator_span,
            operand,
            operand_span,
        } => {
            let rewritten = rewrite_discriminant_aliases(operand, flow_state)?;
            Some(ParsedExpression::Unary {
                operator: *operator,
                operator_span: *operator_span,
                operand: Box::new(rewritten),
                operand_span: *operand_span,
            })
        }
        ParsedExpression::Binary {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let new_left = rewrite_discriminant_aliases(left, flow_state);
            let new_right = rewrite_discriminant_aliases(right, flow_state);
            if new_left.is_none() && new_right.is_none() {
                return None;
            }
            Some(ParsedExpression::Binary {
                left: Box::new(new_left.unwrap_or_else(|| left.as_ref().clone())),
                left_span: *left_span,
                operator: *operator,
                operator_span: *operator_span,
                right: Box::new(new_right.unwrap_or_else(|| right.as_ref().clone())),
                right_span: *right_span,
            })
        }
        ParsedExpression::Logical {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let new_left = rewrite_discriminant_aliases(left, flow_state);
            let new_right = rewrite_discriminant_aliases(right, flow_state);
            if new_left.is_none() && new_right.is_none() {
                return None;
            }
            Some(ParsedExpression::Logical {
                left: Box::new(new_left.unwrap_or_else(|| left.as_ref().clone())),
                left_span: *left_span,
                operator: *operator,
                operator_span: *operator_span,
                right: Box::new(new_right.unwrap_or_else(|| right.as_ref().clone())),
                right_span: *right_span,
            })
        }
        _ => None,
    }
}

/// The condition an `if` really tests: an identifier bound to a boolean `const`
/// guard stands for the expression it was initialized from, also when it is one
/// operand of the condition's `&&`/`||`/`!` structure (`isRefetch && mode ===
/// 'reset'` narrows through `isRefetch`).
pub(super) fn resolved_alias_condition(
    condition: &ParsedExpression,
    flow_state: &FunctionFlowState,
) -> Option<std::sync::Arc<ParsedExpression>> {
    match condition {
        ParsedExpression::Identifier { name, .. } => flow_state.alias_guard_condition(name),
        _ => expand_alias_conditions(condition, flow_state).map(std::sync::Arc::new),
    }
}

pub(super) fn expand_alias_conditions(
    condition: &ParsedExpression,
    flow_state: &FunctionFlowState,
) -> Option<ParsedExpression> {
    match condition {
        ParsedExpression::Identifier { name, .. } => flow_state
            .alias_guard_condition(name)
            .map(|expression| (*expression).clone()),
        ParsedExpression::Logical {
            left,
            left_span,
            operator,
            operator_span,
            right,
            right_span,
        } => {
            let expanded_left = expand_alias_conditions(left, flow_state);
            let expanded_right = expand_alias_conditions(right, flow_state);
            if expanded_left.is_none() && expanded_right.is_none() {
                return None;
            }
            Some(ParsedExpression::Logical {
                left: Box::new(expanded_left.unwrap_or_else(|| (**left).clone())),
                left_span: *left_span,
                operator: *operator,
                operator_span: *operator_span,
                right: Box::new(expanded_right.unwrap_or_else(|| (**right).clone())),
                right_span: *right_span,
            })
        }
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operator_span,
            operand,
            operand_span,
        } => expand_alias_conditions(operand, flow_state).map(|expanded| ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operator_span: *operator_span,
            operand: Box::new(expanded),
            operand_span: *operand_span,
        }),
        _ => None,
    }
}

/// Whether a branch body ends in a call to a `never`-returning function
/// (`process.exit(1)`), which ends control flow exactly as a `return` does.
/// [`analyze_function_body_flow`] is purely syntactic and cannot see a return
/// type, so this one type-dependent case is decided here, where the scope is in
/// hand — without it the fall-through of `if (!args.file) { …; process.exit(1); }`
/// keeps the unnarrowed `string | undefined`.
pub(crate) fn body_ends_in_never_call(body: &[ParsedFunctionBodyStatement], scopes: &ScopeStack) -> bool {
    match body.last() {
        Some(ParsedFunctionBodyStatement::Block(block)) => body_ends_in_never_call(block, scopes),
        Some(ParsedFunctionBodyStatement::Expression(expression)) => {
            call_returns_never(expression, scopes)
        }
        _ => false,
    }
}

/// Whether a call expression's callee is declared to return `never`.
pub(super) fn call_returns_never(expression: &ParsedExpression, scopes: &ScopeStack) -> bool {
    let returns_never = |ty: &Type| {
        matches!(ty, Type::Function(function) if matches!(function.return_type(), Type::Never))
    };
    match expression {
        ParsedExpression::Call { callee_name, .. } => scopes
            .resolve(callee_name)
            .is_some_and(|symbol| returns_never(&symbol.ty)),
        ParsedExpression::PropertyCall {
            object,
            property_name,
            ..
        } => {
            let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
                return false;
            };
            scopes.resolve(name).is_some_and(|symbol| {
                symbol
                    .ty
                    .get_property_access_type(property_name)
                    .is_some_and(|ty| returns_never(&ty))
            })
        }
        _ => false,
    }
}

/// `if (!ok) <exit>` where `ok` is a boolean alias of a guard expression
/// narrows, in the fall-through, the identifiers that alias guarded — dropping a
/// guarded genuine-`unknown` to the degradation sentinel so a later access is
/// not a spurious `TS18046`. Mirrors tsc's aliased-condition narrowing, limited
/// to the genuine-unknown downgrade.
pub(super) fn narrow_aliased_guard_after_exit(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    flow_state: &FunctionFlowState,
) {
    let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    else {
        return;
    };
    let ParsedExpression::Identifier { name, .. } = operand.as_ref() else {
        return;
    };
    if let Some(targets) = flow_state.alias_guard_targets(name) {
        let targets = targets.to_vec();
        downgrade_genuine_unknown_in_scope(&targets, scopes);
    }
}
