//! Truthiness-guard narrowing of identifiers and properties within function bodies.

use std::sync::Arc;
use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, union_type, with_type_copy_reason};

use crate::checks::expr::evaluate_expression;
use crate::checks::ops;
use crate::context::CheckerContext;
use crate::infer::InferredExpression;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};

mod guards;
use guards::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TruthyGuardTarget {
    Identifier(String),
    Property { base: String, property: String },
}

pub(crate) fn narrow_truthy_guarded_identifiers(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
) {
    let mut targets = Vec::new();
    collect_truthy_guarded_identifiers(condition, &mut targets);

    for target in targets {
        let base_name = match &target {
            TruthyGuardTarget::Identifier(name) => name,
            TruthyGuardTarget::Property { base, .. } => base,
        };

        let Some(symbol) = scopes.resolve(base_name) else {
            continue;
        };

        let narrowed = match &target {
            TruthyGuardTarget::Identifier(_) => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    surge_ts_types::remove_undefined(&symbol.ty)
                })
            }
            TruthyGuardTarget::Property { property, .. } => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    narrow_truthy_guarded_property(&symbol.ty, property)
                })
            }
        };

        if narrowed == symbol.ty {
            continue;
        }

        let _ = scopes.update_visible(
            base_name,
            SymbolInfo {
                ty: narrowed,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }
}

pub(crate) fn collect_truthy_guarded_identifiers(
    condition: &ParsedExpression,
    targets: &mut Vec<TruthyGuardTarget>,
) {
    match condition {
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } => {
            collect_truthy_guarded_identifiers(left, targets);
            collect_truthy_guarded_identifiers(right, targets);
        }
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => {
            if let Some(target) = truthy_guard_target(operand) {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn truthy_guard_target(expression: &ParsedExpression) -> Option<TruthyGuardTarget> {
    match expression {
        ParsedExpression::Identifier { name, .. } => {
            Some(TruthyGuardTarget::Identifier(name.clone()))
        }
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            ..
        } => truthy_guard_base_identifier(object).map(|base| TruthyGuardTarget::Property {
            base,
            property: property_name.clone(),
        }),
        ParsedExpression::NonNullAssertion { expression, .. } => truthy_guard_target(expression),
        _ => None,
    }
}

pub(crate) fn truthy_guard_base_identifier(expression: &ParsedExpression) -> Option<String> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some(name.clone()),
        ParsedExpression::NonNullAssertion { expression, .. } => {
            truthy_guard_base_identifier(expression)
        }
        _ => None,
    }
}

pub(crate) fn narrow_truthy_guarded_property(ty: &Type, property: &str) -> Type {
    let narrowed_base = surge_ts_types::remove_undefined(&ty.peeled());

    match narrowed_base {
        Type::Object(mut object_type) => {
            if let Some(existing) = object_type.properties.get(property).cloned() {
                let properties = Arc::make_mut(&mut object_type.properties);
                properties.insert(
                    property.into(),
                    surge_ts_types::ObjectProperty {
                        ty: surge_ts_types::remove_undefined(&existing.ty),
                        optional: false,
                    },
                );
            }

            Type::Object(object_type)
        }
        Type::Union(union) => union_type(
            union
                .types()
                .iter()
                .map(|member| narrow_truthy_guarded_property(member, property))
                .collect(),
        ),
        _ => narrowed_base,
    }
}

/// Narrows `ty` for variable `var_name` under `condition`, returning the narrowed
/// type or `None` when the condition does not constrain `var_name` (or leaves it
/// unchanged). Composes `||` (true branch: union of disjuncts — every disjunct
/// must constrain `var_name`), `&&` (true branch: sequential), and `!`.
fn narrow_type_for_identifier(
    condition: &ParsedExpression,
    var_name: &str,
    ty: &Type,
    branch_is_true: bool,
) -> Option<Type> {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => narrow_type_for_identifier(operand, var_name, ty, !branch_is_true),
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } if branch_is_true => {
            // `A || B` true branch: the value satisfies A or B, so its type is the
            // union of each disjunct's narrowing. A disjunct that does not
            // constrain `var_name` leaves it unconstrained, so the whole guard
            // cannot narrow — bail.
            let left_narrowed = narrow_type_for_identifier(left, var_name, ty, true)?;
            let right_narrowed = narrow_type_for_identifier(right, var_name, ty, true)?;
            Some(union_type(vec![left_narrowed, right_narrowed]))
        }
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::And,
            right,
            ..
        } if branch_is_true => {
            // `A && B` true branch: apply each guard in sequence.
            let after_left =
                narrow_type_for_identifier(left, var_name, ty, true).unwrap_or_else(|| ty.clone());
            Some(
                narrow_type_for_identifier(right, var_name, &after_left, true)
                    .unwrap_or(after_left),
            )
        }
        _ => narrow_single_guard_for_identifier(condition, var_name, ty, branch_is_true),
    }
}

/// Narrows `ty` for `var_name` under a single (non-composite) guard condition.
fn narrow_single_guard_for_identifier(
    condition: &ParsedExpression,
    var_name: &str,
    ty: &Type,
    branch_is_true: bool,
) -> Option<Type> {
    if let Some((ParsedExpression::Identifier { name, .. }, ctor_name)) =
        parse_instanceof_condition(condition).map(|(operand, ctor)| (operand, ctor))
        && name == var_name
    {
        return narrow_union_by_instanceof(ty, ctor_name, branch_is_true);
    }
    if let Some((ParsedExpression::Identifier { name, .. }, tag, eq)) =
        parse_typeof_condition(condition)
        && name == var_name
    {
        return narrow_union_by_typeof(ty, tag, branch_is_true == eq);
    }
    if let Some(ParsedExpression::Identifier { name, .. }) =
        parse_array_isarray_condition(condition)
        && name == var_name
    {
        return narrow_union_by_arrayness(ty, branch_is_true);
    }
    if let Some(ParsedExpression::Identifier { name, .. }) =
        parse_arraybuffer_isview_condition(condition)
        && name == var_name
    {
        return narrow_union_by_arraybufferview(ty, branch_is_true);
    }
    None
}

/// Applies `||`/`&&`-composed guard narrowing in place to a `ScopeStack`. Returns
/// whether `condition` was such a logical composition (so the caller can stop).
fn narrow_logical_guard_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    if !matches!(
        condition,
        ParsedExpression::Logical {
            operator: ParsedLogicalOperator::Or | ParsedLogicalOperator::And,
            ..
        }
    ) {
        return false;
    }

    let mut operand_names = Vec::new();
    collect_guard_operand_identifiers(condition, &mut operand_names);

    for name in operand_names {
        let Some(symbol) = scopes.resolve(&name) else {
            continue;
        };
        let Some(narrowed) =
            narrow_type_for_identifier(condition, &name, &symbol.ty, branch_is_true)
        else {
            continue;
        };
        if narrowed == symbol.ty {
            continue;
        }
        let narrowed_symbol = SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        };
        let _ = scopes.insert_current(name, narrowed_symbol);
    }
    true
}

/// Collects the distinct identifiers tested by single guards within a
/// (possibly `||`/`&&`/`!`-composed) condition.
/// The identifiers a condition guards as whole values: the `typeof`/`in`/
/// `instanceof`/`Array.isArray` operands plus any bare-truthy identifier across
/// an `&&` chain. A guard on such an identifier removes its genuine-unknownness
/// in the branch where the condition holds (property bases like `x.p` are
/// excluded — guarding `x.p` does not narrow `x`).
pub(crate) fn guarded_value_identifiers(condition: &ParsedExpression) -> Vec<String> {
    let mut names = Vec::new();
    collect_guard_operand_identifiers(condition, &mut names);

    let mut truthy = Vec::new();
    collect_and_chain_truthy_targets(condition, &mut truthy);
    for target in truthy {
        if let TruthyGuardTarget::Identifier(name) = target {
            if !names.iter().any(|existing| existing == &name) {
                names.push(name);
            }
        }
    }
    names
}

/// Drops a guarded [`Type::GenuineUnknown`] identifier to [`Type::Unknown`] in a
/// scope stack, so an access inside the guarded branch is not reported as
/// `TS18046`.
fn downgrade_guarded_genuine_unknown_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
) {
    downgrade_genuine_unknown_in_scope(&guarded_value_identifiers(condition), scopes);
}

/// Drops each named [`Type::GenuineUnknown`] binding to [`Type::Unknown`] in a
/// scope stack, so an access in the guarded branch is not reported as `TS18046`.
pub(crate) fn downgrade_genuine_unknown_in_scope(names: &[String], scopes: &mut ScopeStack) {
    for name in names {
        let downgraded = scopes.resolve(name).and_then(|symbol| {
            matches!(symbol.ty, Type::GenuineUnknown).then(|| SymbolInfo {
                ty: Type::Unknown,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            })
        });
        if let Some(downgraded) = downgraded {
            let _ = scopes.insert_current(name.clone(), downgraded);
        }
    }
}

fn collect_guard_operand_identifiers(condition: &ParsedExpression, names: &mut Vec<String>) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_guard_operand_identifiers(operand, names),
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or | ParsedLogicalOperator::And,
            right,
            ..
        } => {
            collect_guard_operand_identifiers(left, names);
            collect_guard_operand_identifiers(right, names);
        }
        _ => {
            if let Some(name) = guard_operand_identifier(condition)
                && !names.iter().any(|existing| existing == name)
            {
                names.push(name.to_string());
            }
        }
    }
}

/// Narrows for a branch by any recognized type guard: a discriminated-union
/// equality test (`x.kind === "a"`), a `typeof x === "tag"` test, an
/// `x instanceof Ctor` test, an `Array.isArray(x)` test, or an `in`
/// property-presence test (`"prop" in x`). Returns `None` if none apply.
pub(crate) fn narrow_condition_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    // `!guard` narrows the opposite branch.
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        return narrow_condition_symbol_table(operand, symbols, !branch_is_true);
    }

    narrow_discriminant_symbol_table(condition, symbols, branch_is_true)
        .or_else(|| narrow_typeof_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_instanceof_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_array_isarray_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_property_presence_symbol_table(condition, symbols, branch_is_true))
}

/// Collects the identifiers/properties an `&&` chain proves truthy, so the right
/// side of `a.b && a.b > c` (and the then-branch) sees them non-nullish.
fn collect_and_chain_truthy_targets(
    condition: &ParsedExpression,
    targets: &mut Vec<TruthyGuardTarget>,
) {
    if let ParsedExpression::Logical {
        left,
        operator: ParsedLogicalOperator::And,
        right,
        ..
    } = condition
    {
        collect_and_chain_truthy_targets(left, targets);
        collect_and_chain_truthy_targets(right, targets);
    } else if let Some(target) = truthy_guard_target(condition) {
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
}

/// Narrows `symbols` by everything the truthy operand of an `&&` proves: a
/// structured guard (`x.kind === "k" && …`) plus each identifier/property in the
/// `&&` chain narrowed to non-nullish (`a.b && a.b > c`). Returns `None` when
/// nothing narrows. Used to type the right operand of `&&`.
pub(crate) fn narrow_truthy_operand_symbol_table(
    operand: &ParsedExpression,
    symbols: &SymbolTable,
) -> Option<SymbolTable> {
    let structured = narrow_condition_symbol_table(operand, symbols, true);

    let mut targets = Vec::new();
    collect_and_chain_truthy_targets(operand, &mut targets);

    // Identifiers the left side guards as a whole value — `typeof x === "object"`,
    // `"p" in x`, `x instanceof C`, `Array.isArray(x)`, and a bare truthy `x`.
    // A guard on a genuinely-`unknown` value (`x: unknown`) makes tsc narrow it,
    // so a later access (`typeof x === "object" && "p" in x && x.p`) is not a
    // `TS18046`. surge does not compute the narrowed shape for `unknown`, but it
    // must at least stop treating the value as a genuine-unknown receiver — drop
    // it to the degradation sentinel so the property-access check stays silent,
    // matching tsc's no-cascade behavior.
    let mut guarded_identifiers = Vec::new();
    collect_guard_operand_identifiers(operand, &mut guarded_identifiers);
    for target in &targets {
        if let TruthyGuardTarget::Identifier(name) = target {
            if !guarded_identifiers.iter().any(|existing| existing == name) {
                guarded_identifiers.push(name.clone());
            }
        }
    }

    if targets.is_empty() && guarded_identifiers.is_empty() {
        return structured;
    }

    let base = structured.as_ref().unwrap_or(symbols);
    let mut narrowed = base.clone_with_reason(TypeCopyReason::ScopeOrContext);
    let mut changed = structured.is_some();

    for name in &guarded_identifiers {
        let downgraded = narrowed.get(name).and_then(|symbol| {
            matches!(symbol.ty, Type::GenuineUnknown).then(|| SymbolInfo {
                ty: Type::Unknown,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            })
        });
        if let Some(downgraded) = downgraded {
            narrowed.insert(name.clone(), downgraded);
            changed = true;
        }
    }
    for target in targets {
        let base_name = match &target {
            TruthyGuardTarget::Identifier(name) => name,
            TruthyGuardTarget::Property { base, .. } => base,
        };
        let Some(symbol) = narrowed.get(base_name) else {
            continue;
        };
        let new_ty = match &target {
            TruthyGuardTarget::Identifier(_) => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    surge_ts_types::remove_nullish(&symbol.ty)
                })
            }
            TruthyGuardTarget::Property { property, .. } => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    narrow_truthy_guarded_property(&symbol.ty, property)
                })
            }
        };
        if new_ty == symbol.ty {
            continue;
        }
        changed = true;
        narrowed.insert(
            base_name.clone(),
            SymbolInfo {
                ty: new_ty,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }

    changed.then_some(narrowed)
}

/// Like [`narrow_discriminant_symbol_table`] but applies the narrowing in place
/// to a `ScopeStack`, for narrowing a discriminated union inside an `if` branch
/// (or after an early-returning `if`).
pub(crate) fn narrow_discriminant_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) {
    // `!guard` narrows the opposite branch.
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        narrow_discriminant_in_scope(operand, scopes, !branch_is_true);
        return;
    }

    // In the branch where the condition holds, a guard on a genuinely-`unknown`
    // value narrows it (tsc), so a later access inside the branch is not a
    // `TS18046`. Drop the guarded identifier to the degradation sentinel so the
    // property-access check stays silent. See the matching `&&`-operand path in
    // `narrow_truthy_operand_symbol_table`.
    if branch_is_true {
        downgrade_guarded_genuine_unknown_in_scope(condition, scopes);
    }

    if narrow_logical_guard_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_typeof_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_instanceof_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_array_isarray_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_arraybuffer_isview_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_truthy_identifier_in_scope(condition, scopes, branch_is_true) {
        return;
    }

    let Some((discriminant_object, property, literal, eq)) =
        parse_discriminant_condition(condition)
    else {
        return;
    };
    let keep_matching = branch_is_true == eq;

    let (base_name, narrowed_symbol) = match discriminant_object {
        ParsedExpression::Identifier { name, .. } => {
            let Some(symbol) = scopes.resolve(name) else {
                return;
            };
            let Some(narrowed) =
                narrow_union_by_discriminant(&symbol.ty, property, &literal, keep_matching)
            else {
                return;
            };
            (
                name.clone(),
                SymbolInfo {
                    ty: narrowed,
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
            )
        }
        ParsedExpression::PropertyAccess {
            object,
            property_name: base_property,
            ..
        } => {
            let ParsedExpression::Identifier { name, .. } = object.as_ref() else {
                return;
            };
            let Some(symbol) = scopes.resolve(name) else {
                return;
            };
            // Peel a named-typed base (`draft: Draft`) to narrow its discriminant
            // property in scope.
            let symbol_ty = symbol.ty.peeled();
            let Type::Object(object_type) = &symbol_ty else {
                return;
            };
            let Some(base_property_type) = object_type.properties.get(base_property.as_str())
            else {
                return;
            };
            let Some(narrowed_property) = narrow_union_by_discriminant(
                &base_property_type.ty,
                property,
                &literal,
                keep_matching,
            ) else {
                return;
            };
            let mut new_object = object_type.clone();
            let properties = std::sync::Arc::make_mut(&mut new_object.properties);
            properties.insert(
                base_property.as_str().into(),
                surge_ts_types::ObjectProperty {
                    ty: narrowed_property,
                    optional: base_property_type.optional,
                },
            );
            (
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(new_object),
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
            )
        }
        _ => return,
    };

    // Insert into the *current* frame (shadowing the binding's owner) rather than
    // mutating the owning frame: the then-branch narrowing must stay confined to
    // its pushed child scope, or it leaks into the parent and corrupts the
    // else/fall-through branch (which would then narrow an already-narrowed
    // non-union to nothing). `pop_child` restores the shadow.
    let _ = scopes.insert_current(base_name, narrowed_symbol);
}

/// Positive truthy narrowing of a bare-identifier guard (`if (x) { … }`): the
/// true branch drops the nullish members (`undefined`/`void`) the truthy test
/// excludes, so a `T | undefined` callee/value resolves to `T` inside the block.
/// Only the true branch narrows; the falsy complement (`""`, `0`, `false`, …) is
/// not modelled, so the false branch keeps the original type. The `!guard` unwrap
/// in [`narrow_discriminant_in_scope`] routes `if (!x)` else/fall-through here
/// with `branch_is_true` already flipped. Returns whether the condition was a
/// bare identifier (always handled, so equality-discriminant parsing is skipped).
fn narrow_truthy_identifier_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let ParsedExpression::Identifier { name, .. } = condition else {
        return false;
    };
    if !branch_is_true {
        return true;
    }
    let Some(symbol) = scopes.resolve(name) else {
        return true;
    };
    let narrowed = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        surge_ts_types::remove_nullish(&symbol.ty)
    });
    if narrowed == symbol.ty {
        return true;
    }
    let _ = scopes.insert_current(
        name.clone(),
        SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
    );
    true
}

/// Applies `typeof x === "tag"` narrowing in place to a `ScopeStack` (the if-body
/// and early-return paths). Returns whether the condition was a typeof guard.
fn narrow_typeof_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((operand, tag, eq)) = parse_typeof_condition(condition) else {
        return false;
    };
    let ParsedExpression::Identifier { name, .. } = operand else {
        return false;
    };
    let Some(symbol) = scopes.resolve(name) else {
        return true;
    };
    let Some(narrowed) = narrow_union_by_typeof(&symbol.ty, tag, branch_is_true == eq) else {
        return true;
    };
    let narrowed_symbol = SymbolInfo {
        ty: narrowed,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let name = name.clone();
    // Shadow in the current frame, not the owning frame — see the note in
    // `narrow_discriminant_in_scope`.
    let _ = scopes.insert_current(name, narrowed_symbol);
    true
}

/// Applies `x instanceof Ctor` narrowing in place to a `ScopeStack`. Returns
/// whether the condition was an instanceof guard over a bare identifier.
fn narrow_instanceof_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((operand, ctor_name)) = parse_instanceof_condition(condition) else {
        return false;
    };
    let ParsedExpression::Identifier { name, .. } = operand else {
        return false;
    };
    let Some(symbol) = scopes.resolve(name) else {
        return true;
    };
    let Some(narrowed) = narrow_union_by_instanceof(&symbol.ty, ctor_name, branch_is_true) else {
        return true;
    };
    let narrowed_symbol = SymbolInfo {
        ty: narrowed,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let name = name.clone();
    let _ = scopes.insert_current(name, narrowed_symbol);
    true
}

/// Applies `Array.isArray(x)` narrowing in place to a `ScopeStack`. Returns
/// whether the condition was such a guard over a bare identifier.
fn narrow_array_isarray_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some(operand) = parse_array_isarray_condition(condition) else {
        return false;
    };
    let ParsedExpression::Identifier { name, .. } = operand else {
        return false;
    };
    let Some(symbol) = scopes.resolve(name) else {
        return true;
    };
    let Some(narrowed) = narrow_union_by_arrayness(&symbol.ty, branch_is_true) else {
        return true;
    };
    let narrowed_symbol = SymbolInfo {
        ty: narrowed,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let name = name.clone();
    let _ = scopes.insert_current(name, narrowed_symbol);
    true
}

/// Applies `ArrayBuffer.isView(x)` narrowing in place to a `ScopeStack`. Returns
/// whether the condition was such a guard over a bare identifier.
fn narrow_arraybuffer_isview_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some(operand) = parse_arraybuffer_isview_condition(condition) else {
        return false;
    };
    let ParsedExpression::Identifier { name, .. } = operand else {
        return false;
    };
    let Some(symbol) = scopes.resolve(name) else {
        return true;
    };
    let Some(narrowed) = narrow_union_by_arraybufferview(&symbol.ty, branch_is_true) else {
        return true;
    };
    let narrowed_symbol = SymbolInfo {
        ty: narrowed,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let name = name.clone();
    let _ = scopes.insert_current(name, narrowed_symbol);
    true
}

pub(crate) fn evaluate_condition_expression_with_truthy_guards(
    expression: &ParsedExpression,
    fallback_span: Option<surge_ts_syntax::TextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> InferredExpression {
    match expression {
        ParsedExpression::Logical {
            left,
            left_span,
            operator: ParsedLogicalOperator::Or,
            right,
            right_span,
            ..
        } => {
            let left_result = evaluate_condition_expression_with_truthy_guards(
                left,
                left_span.or(fallback_span),
                symbols,
                ctx,
            );
            let narrowed_symbols = narrow_truthy_guarded_symbol_table(left, symbols);
            let right_result = evaluate_condition_expression_with_truthy_guards(
                right,
                right_span.or(fallback_span),
                &narrowed_symbols,
                ctx,
            );
            ops::evaluate_logical_expression(ParsedLogicalOperator::Or, left_result, right_result)
        }
        _ => evaluate_expression(expression, fallback_span, symbols, ctx),
    }
}

pub(crate) fn narrow_truthy_guarded_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
) -> SymbolTable {
    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    let mut targets = Vec::new();
    collect_truthy_guarded_identifiers(condition, &mut targets);

    for target in targets {
        let base_name = match &target {
            TruthyGuardTarget::Identifier(name) => name,
            TruthyGuardTarget::Property { base, .. } => base,
        };

        let Some(symbol) = narrowed_symbols.get(base_name) else {
            continue;
        };

        let narrowed = match &target {
            TruthyGuardTarget::Identifier(_) => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    surge_ts_types::remove_undefined(&symbol.ty)
                })
            }
            TruthyGuardTarget::Property { property, .. } => {
                with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
                    narrow_truthy_guarded_property(&symbol.ty, property)
                })
            }
        };

        if narrowed == symbol.ty {
            continue;
        }

        narrowed_symbols.insert(
            base_name.clone(),
            SymbolInfo {
                ty: narrowed,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }

    narrowed_symbols
}

#[cfg(test)]
mod tests {
    use super::*;
    use surge_ts_types::union_type;

    fn union3() -> Type {
        union_type(vec![
            Type::String,
            Type::Number,
            Type::Array(Box::new(Type::Number)),
        ])
    }

    #[test]
    fn typeof_keeps_matching_member_in_true_branch() {
        let narrowed = narrow_union_by_typeof(&union3(), "number", true).unwrap();
        assert_eq!(narrowed, Type::Number);
    }

    #[test]
    fn typeof_removes_matching_member_in_false_branch() {
        let narrowed = narrow_union_by_typeof(&union3(), "string", false).unwrap();
        assert_eq!(
            narrowed,
            union_type(vec![Type::Number, Type::Array(Box::new(Type::Number))])
        );
    }

    #[test]
    fn arrayness_keeps_array_member_in_true_branch() {
        let narrowed = narrow_union_by_arrayness(&union3(), true).unwrap();
        assert_eq!(narrowed, Type::Array(Box::new(Type::Number)));
    }

    #[test]
    fn arrayness_removes_array_member_in_false_branch() {
        let narrowed = narrow_union_by_arrayness(&union3(), false).unwrap();
        assert_eq!(narrowed, union_type(vec![Type::String, Type::Number]));
    }

    #[test]
    fn narrowing_returns_none_when_nothing_changes() {
        // No member has tag "boolean", so the true branch would be empty -> None.
        assert!(narrow_union_by_typeof(&union3(), "boolean", true).is_none());
        // A non-union type is never narrowed.
        assert!(narrow_union_by_arrayness(&Type::String, true).is_none());
    }

    struct FixedResolver(Type);
    impl surge_ts_types::ResolveReference for FixedResolver {
        fn resolve(&self) -> Type {
            self.0.clone()
        }
    }

    fn view_reference(display: &str) -> Type {
        Type::Reference(surge_ts_types::TypeReference::new(
            format!("lib.dom.d.ts\u{0}{}", display.split('<').next().unwrap()),
            display,
            Vec::new(),
            std::sync::Arc::new(FixedResolver(Type::Unknown)),
        ))
    }

    #[test]
    fn arraybufferview_keeps_view_members_in_true_branch() {
        // `ArrayBuffer.isView(x)` keeps the `ArrayBufferView<ArrayBuffer>` member
        // and drops the `ArrayBuffer` / primitive members.
        let union = union_type(vec![
            view_reference("ArrayBufferView<ArrayBuffer>"),
            view_reference("ArrayBuffer"),
            Type::String,
        ]);
        let narrowed = narrow_union_by_arraybufferview(&union, true).unwrap();
        assert_eq!(narrowed, view_reference("ArrayBufferView<ArrayBuffer>"));
    }

    #[test]
    fn arraybufferview_removes_view_members_in_false_branch() {
        let union = union_type(vec![
            view_reference("Int8Array"),
            view_reference("ArrayBuffer"),
            Type::String,
        ]);
        let narrowed = narrow_union_by_arraybufferview(&union, false).unwrap();
        assert_eq!(
            narrowed,
            union_type(vec![view_reference("ArrayBuffer"), Type::String])
        );
    }
}
