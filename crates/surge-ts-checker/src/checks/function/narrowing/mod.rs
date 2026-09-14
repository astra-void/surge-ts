//! Truthiness-guard narrowing of identifiers and properties within function bodies.

use std::sync::Arc;
use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, union_type};

use crate::context::CheckerContext;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};

mod element_reference;
mod guards;
mod predicate;
mod reference;
mod truthy;
mod type_guards;

pub(crate) use element_reference::*;
use guards::*;
pub(crate) use predicate::*;
pub(crate) use reference::*;
pub(crate) use truthy::*;
pub(crate) use type_guards::*;

/// Narrows `ty` for variable `var_name` under `condition`, returning the narrowed
/// type or `None` when the condition does not constrain `var_name` (or leaves it
/// unchanged). Composes `||` (true branch: union of disjuncts — every disjunct
/// must constrain `var_name`), `&&` (true branch: sequential), and `!`.
fn narrow_type_for_identifier(
    condition: &ParsedExpression,
    var_name: &str,
    ty: &Type,
    branch_is_true: bool,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => narrow_type_for_identifier(operand, var_name, ty, !branch_is_true, scopes, ctx),
        _ if strip_boolean_literal_comparison(condition).is_some() => {
            let (inner, flip) = strip_boolean_literal_comparison(condition)
                .expect("boolean-literal comparison checked above");
            narrow_type_for_identifier(inner, var_name, ty, branch_is_true != flip, scopes, ctx)
        }
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
            let left_narrowed = narrow_type_for_identifier(left, var_name, ty, true, scopes, ctx)?;
            let right_narrowed =
                narrow_type_for_identifier(right, var_name, ty, true, scopes, ctx)?;
            Some(union_type(vec![left_narrowed, right_narrowed]))
        }
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::And,
            right,
            ..
        } if branch_is_true => {
            // `A && B` true branch: apply each guard in sequence.
            let after_left = narrow_type_for_identifier(left, var_name, ty, true, scopes, ctx)
                .unwrap_or_else(|| ty.clone());
            Some(
                narrow_type_for_identifier(right, var_name, &after_left, true, scopes, ctx)
                    .unwrap_or(after_left),
            )
        }
        // Fall-through of `A || B` is `!A && !B`: apply each disjunct's negation
        // in sequence. Treating the whole guard as one atom here narrowed to the
        // disjunct's *true* shape, so `if (r.kind === "x" || other) continue;`
        // left `r` as the `"x"` member instead of removing it.
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } => {
            let after_left = narrow_type_for_identifier(left, var_name, ty, false, scopes, ctx)
                .unwrap_or_else(|| ty.clone());
            Some(
                narrow_type_for_identifier(right, var_name, &after_left, false, scopes, ctx)
                    .unwrap_or(after_left),
            )
        }
        // Fall-through of `A && B` is `!A || !B` — a union, and only sound when
        // both operands constrain the value.
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::And,
            right,
            ..
        } => {
            let left_narrowed = narrow_type_for_identifier(left, var_name, ty, false, scopes, ctx)?;
            let right_narrowed =
                narrow_type_for_identifier(right, var_name, ty, false, scopes, ctx)?;
            Some(union_type(vec![left_narrowed, right_narrowed]))
        }
        _ => {
            narrow_single_guard_for_identifier(condition, var_name, ty, branch_is_true, scopes, ctx)
        }
    }
}

fn narrow_single_guard_for_identifier(
    condition: &ParsedExpression,
    var_name: &str,
    ty: &Type,
    branch_is_true: bool,
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    if let Some((ParsedExpression::Identifier { name, .. }, ctor_name)) =
        parse_instanceof_condition(condition).map(|(operand, ctor)| (operand, ctor))
        && name == var_name
    {
        let instance = resolve_constructor_instance_type(ctor_name, ctx);
        if let Some(narrowed) =
            narrow_union_by_instanceof(ty, ctor_name, instance.as_ref(), branch_is_true)
        {
            return Some(narrowed);
        }
        return narrow_to_instanceof_subclass(ty, instance.as_ref(), branch_is_true);
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
    if let Some(guard) = parse_type_predicate_condition(condition, &mut |callee| {
        predicate_callee_signature(
            callee,
            |name| scopes.resolve(name),
            scopes.visible_symbols(),
            ctx,
        )
    }) && guard.subject == var_name
    {
        return match resolve_predicate_guard_target(
            &guard,
            Some(ty),
            scopes.visible_symbols(),
            ctx,
        )? {
            PredicateTarget::Resolved(predicate_ty) => {
                narrow_by_predicate(ty, &predicate_ty, branch_is_true)
            }
            PredicateTarget::Degraded => degraded_predicate_subject(branch_is_true),
        };
    }
    if let Some((subject, path, method)) = parse_this_predicate_call(condition)
        && path.is_empty()
        && subject == var_name
        && let Some(target) = this_predicate_target(ty, method, ctx)
    {
        return narrow_by_predicate(ty, &target, branch_is_true);
    }
    if let Some((ParsedExpression::Identifier { name, .. }, property, literal, eq)) =
        parse_discriminant_condition_with(condition, &|expression| {
            const_member_literal_value(expression, scopes.visible_symbols())
        })
        && name == var_name
    {
        let keep_matching = branch_is_true == eq;
        return narrow_union_by_discriminant(ty, property, &literal, keep_matching)
            .or_else(|| narrow_optional_chain_base(condition, ty, &literal, keep_matching));
    }
    if let Some((ParsedExpression::Identifier { name, .. }, eq)) =
        parse_nullish_equality_condition(condition)
        && name == var_name
    {
        return narrow_union_by_nullish(ty, branch_is_true == eq);
    }
    if let Some((name, literal, eq)) = parse_identifier_literal_equality(condition)
        && name == var_name
    {
        return narrow_by_literal_equality(ty, &literal, branch_is_true == eq);
    }
    if let Some((ParsedExpression::Identifier { name, .. }, property)) =
        parse_in_condition(condition)
        && name == var_name
    {
        return narrow_union_by_property_presence(ty, property, branch_is_true);
    }
    None
}

/// Applies `||`/`&&`-composed guard narrowing in place to a `ScopeStack`. Returns
/// whether `condition` was such a logical composition (so the caller can stop).
fn narrow_logical_guard_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
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
    collect_predicate_guard_subjects(condition, scopes, &mut operand_names, ctx);
    collect_equality_guard_subjects(condition, scopes, &mut operand_names);

    for name in operand_names {
        let Some(symbol) = scopes.resolve(&name) else {
            continue;
        };
        let symbol_ty = symbol.ty.clone();
        let kind = symbol.kind;
        let function_signature = symbol.function_signature.clone();
        let Some(narrowed) =
            narrow_type_for_identifier(condition, &name, &symbol_ty, branch_is_true, scopes, ctx)
        else {
            continue;
        };
        if narrowed == symbol_ty {
            continue;
        }
        let narrowed_symbol = SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        };
        let _ = scopes.insert_current_narrowed(name, narrowed_symbol, symbol_ty);
    }

    // Every operand of an `&&` holds in its true branch, so the chain also proves
    // each truthy-tested reference (`o.p && o.p.q`) non-nullish — which
    // `narrow_type_for_identifier` above, keyed on a whole binding, cannot express.
    let mut reference_guards = Vec::new();
    collect_reference_guards(condition, branch_is_true, &mut reference_guards);
    for (base, path, guard) in reference_guards {
        narrow_reference_in_scope(&base, &path, guard, scopes);
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

/// Identifiers a condition *proves* guarded in `branch_is_true`. Unlike
/// [`collect_guard_operand_identifiers`], which ignores polarity, this walks the
/// operand shapes whose individual outcomes the branch pins down: `!`, the `&&`
/// operands of a true branch, and the `||` operands of a false one. That last
/// case is what the fall-through of `if (typeof x !== "object" || !("p" in x))
/// return;` needs — both disjuncts are false there, so both guards hold.
///
/// The true branch keeps the historical permissive reading of `A || B` (either
/// disjunct counts), since tightening it would turn suppressed `TS18046`s into
/// new false positives rather than removing any.
pub(crate) fn collect_holding_guard_identifiers(
    condition: &ParsedExpression,
    branch_is_true: bool,
    names: &mut Vec<String>,
) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_holding_guard_identifiers(operand, !branch_is_true, names),
        _ if strip_boolean_literal_comparison(condition).is_some() => {
            let (inner, flip) = strip_boolean_literal_comparison(condition)
                .expect("boolean-literal comparison checked above");
            collect_holding_guard_identifiers(inner, branch_is_true != flip, names);
        }
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } if !branch_is_true => {
            collect_holding_guard_identifiers(left, false, names);
            collect_holding_guard_identifiers(right, false, names);
        }
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::And,
            right,
            ..
        } if branch_is_true => {
            collect_holding_guard_identifiers(left, true, names);
            collect_holding_guard_identifiers(right, true, names);
        }
        ParsedExpression::Logical {
            operator: ParsedLogicalOperator::Or,
            ..
        } if branch_is_true => collect_guard_operand_identifiers(condition, names),
        ParsedExpression::Logical { .. } => {}
        _ => {
            let name = if branch_is_true {
                guard_operand_identifier(condition).map(str::to_string)
            } else {
                // `typeof x !== "tag"` being false is itself a guard that holds.
                match parse_typeof_condition(condition) {
                    Some((ParsedExpression::Identifier { name, .. }, _, false)) => {
                        Some(name.clone())
                    }
                    _ => None,
                }
            };
            if let Some(name) = name
                && !names.iter().any(|existing| *existing == name)
            {
                names.push(name);
            }
        }
    }
}

/// Narrows for a branch by any recognized type guard: a discriminated-union
/// equality test (`x.kind === "a"`), a `typeof x === "tag"` test, an
/// `x instanceof Ctor` test, an `Array.isArray(x)` test, an `in`
/// property-presence test (`"prop" in x`), or a nullish equality test
/// (`x !== undefined`). Returns `None` if none apply.
pub(crate) fn narrow_condition_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    // `ok ? a : b` where `ok` is a boolean `const` alias narrows by the
    // condition the alias was written as, not by the opaque identifier.
    if let ParsedExpression::Identifier { name, .. } = condition
        && let Some(alias) = symbols.alias_condition(name)
    {
        return narrow_condition_symbol_table(&alias, symbols, branch_is_true);
    }
    // `!guard` narrows the opposite branch.
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        return narrow_condition_symbol_table(operand, symbols, !branch_is_true);
    }
    if let Some((inner, flip)) = strip_boolean_literal_comparison(condition) {
        return narrow_condition_symbol_table(inner, symbols, branch_is_true != flip);
    }

    // Every operand of an `&&` holds in its true branch, so a chain narrows by
    // all of them (`a !== undefined && b !== undefined && a <= b`). The false
    // branch proves nothing about any individual operand.
    if branch_is_true
        && let ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::And,
            right,
            ..
        } = condition
    {
        let left_narrowed = narrow_condition_symbol_table(left, symbols, true);
        let base = left_narrowed.as_ref().unwrap_or(symbols);
        return narrow_condition_symbol_table(right, base, true).or(left_narrowed);
    }

    // Every operand of an `||` fails in its false branch, so the chain narrows
    // by all of them (`!a || !b || a.x` reads `a.x` with both present).
    if !branch_is_true
        && let ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } = condition
    {
        let left_narrowed = narrow_condition_symbol_table(left, symbols, false);
        let base = left_narrowed.as_ref().unwrap_or(symbols);
        return narrow_condition_symbol_table(right, base, false).or(left_narrowed);
    }

    // `A || B` holds when either disjunct does, so the subject is the union of
    // what each proves. A disjunct that narrows nothing leaves the subject
    // unconstrained, and the union collapses back to the declared type — which
    // is why both sides have to narrow for this to say anything.
    if branch_is_true
        && let ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or,
            right,
            ..
        } = condition
    {
        return union_disjunct_narrowings(
            symbols,
            &narrow_condition_symbol_table(left, symbols, true)?,
            &narrow_condition_symbol_table(right, symbols, true)?,
        );
    }

    narrow_discriminant_symbol_table(condition, symbols, branch_is_true)
        .or_else(|| narrow_literal_equality_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_typeof_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_instanceof_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_array_isarray_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_property_presence_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_nullish_equality_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_reference_guard_symbol_table(condition, symbols, branch_is_true))
}

/// Merges the two disjunct narrowings of an `A || B` true branch: each name
/// either side narrowed becomes the union of what both leave it as.
fn union_disjunct_narrowings(
    symbols: &SymbolTable,
    left: &SymbolTable,
    right: &SymbolTable,
) -> Option<SymbolTable> {
    let mut names: Vec<Arc<str>> = left.narrowed_names().cloned().collect();
    for name in right.narrowed_names() {
        if !names.contains(name) {
            names.push(Arc::clone(name));
        }
    }

    let mut merged = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    let mut changed = false;
    for name in names {
        let (Some(declared), Some(left_symbol), Some(right_symbol)) =
            (symbols.get(&name), left.get(&name), right.get(&name))
        else {
            continue;
        };
        let narrowed = union_type(vec![left_symbol.ty.clone(), right_symbol.ty.clone()]);
        if narrowed == declared.ty {
            continue;
        }
        let declared_ty = declared.ty.clone();
        let narrowed_symbol = SymbolInfo {
            ty: narrowed,
            kind: declared.kind,
            function_signature: declared.function_signature.clone(),
        };
        merged.insert_narrowed(Arc::clone(&name), narrowed_symbol, declared_ty);
        changed = true;
    }

    changed.then_some(merged)
}

/// Removes `null`/`undefined` from a reference's type in the current scope.
/// `for (const _ in ref)` is the one statement form that narrows this way
/// without a condition (tsc, flow.go: "for (const _ in ref) acts as a nonnull
/// on ref").
pub(crate) fn narrow_reference_non_null_in_scope(
    expression: &ParsedExpression,
    scopes: &mut ScopeStack,
) {
    let Some((base, path)) = reference_path(expression) else {
        return;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::Nullish {
            keep_matching: false,
        },
        scopes,
    );
}

fn narrow_value_guards_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) {
    // `!guard` narrows the opposite branch.
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        narrow_value_guards_in_scope(operand, scopes, !branch_is_true, ctx);
        return;
    }
    if let Some((inner, flip)) = strip_boolean_literal_comparison(condition) {
        narrow_value_guards_in_scope(inner, scopes, branch_is_true != flip, ctx);
        return;
    }

    // In the branch where the condition holds, a guard on a genuinely-`unknown`
    // value narrows it (tsc), so a later access inside the branch is not a
    // `TS18046`. Drop the guarded identifier to the degradation sentinel so the
    // property-access check stays silent. See the matching `&&`-operand path in
    // `narrow_truthy_operand_symbol_table`.
    if branch_is_true {
        downgrade_guarded_genuine_unknown_in_scope(condition, scopes);
    } else {
        let mut names = Vec::new();
        collect_holding_guard_identifiers(condition, false, &mut names);
        downgrade_genuine_unknown_in_scope(&names, scopes);
    }

    narrow_element_reference_guards_in_scope(condition, scopes, branch_is_true, ctx);

    if narrow_logical_guard_in_scope(condition, scopes, branch_is_true, ctx) {
        return;
    }
    if narrow_typeof_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_instanceof_in_scope(condition, scopes, branch_is_true, ctx) {
        return;
    }
    if narrow_array_isarray_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_arraybuffer_isview_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_property_presence_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_predicate_call_in_scope(condition, scopes, branch_is_true, ctx) {
        return;
    }
    if narrow_this_predicate_call_in_scope(condition, scopes, branch_is_true, ctx) {
        return;
    }
    if narrow_truthy_reference_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_nullish_equality_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_literal_equality_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    // Narrows the property itself; the discriminant test below then still
    // filters the base union by the same condition, so the two compose.
    narrow_literal_equality_reference_in_scope(condition, scopes, branch_is_true);

    let parsed = {
        let symbols = scopes.visible_symbols();
        parse_discriminant_condition_with(condition, &|expression| {
            const_member_literal_value(expression, symbols)
        })
    };
    let Some((discriminant_object, property, literal, eq)) = parsed else {
        return;
    };
    let keep_matching = branch_is_true == eq;

    let (base_name, narrowed_symbol, declared) = match discriminant_object {
        ParsedExpression::Identifier { name, .. } => {
            let Some(symbol) = scopes.resolve(name) else {
                return;
            };
            let Some(narrowed) =
                narrow_union_by_discriminant(&symbol.ty, property, &literal, keep_matching)
                    .or_else(|| {
                        narrow_optional_chain_base(condition, &symbol.ty, &literal, keep_matching)
                    })
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
                symbol.ty.clone(),
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
                    method: base_property_type.method,
                },
            );
            (
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(new_object),
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                },
                symbol.ty.clone(),
            )
        }
        _ => return,
    };

    // Insert into the *current* frame (shadowing the binding's owner) rather than
    // mutating the owning frame: the then-branch narrowing must stay confined to
    // its pushed child scope, or it leaks into the parent and corrupts the
    // else/fall-through branch (which would then narrow an already-narrowed
    // non-union to nothing). `pop_child` restores the shadow.
    let _ = scopes.insert_current_narrowed(base_name, narrowed_symbol, declared);
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
        // A non-union type is never narrowed.
        assert!(narrow_union_by_arrayness(&Type::String, true).is_none());
    }

    #[test]
    fn typeof_narrows_an_unreachable_branch_to_never() {
        // No member has tag "boolean", so nothing can reach the true branch and
        // tsc types the subject `never` there — not "unchanged".
        assert_eq!(
            narrow_union_by_typeof(&union3(), "boolean", true),
            Some(Type::Never)
        );
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

    fn object(properties: &[(&str, bool)]) -> Type {
        let mut map = surge_ts_types::PropertyMap::default();
        for (name, optional) in properties {
            map.insert(
                (*name).into(),
                surge_ts_types::ObjectProperty {
                    ty: Type::Number,
                    optional: *optional,
                    method: false,
                },
            );
        }
        Type::Object(surge_ts_types::ObjectType::new(map, None))
    }

    fn named_reference(display: &str, target: Type) -> Type {
        Type::Reference(surge_ts_types::TypeReference::new(
            format!("test.ts\u{0}{display}"),
            display,
            Vec::new(),
            std::sync::Arc::new(FixedResolver(target)),
        ))
    }

    #[test]
    fn property_presence_narrows_through_nominal_references() {
        // `Named` is an interface/alias wrapper, so the union has no bare
        // `Type::Object` member to decide on without peeling.
        let named = named_reference("Named", object(&[("a", false)]));
        let other = named_reference("Other", object(&[("b", false)]));
        let union = union_type(vec![named.clone(), other.clone()]);

        assert_eq!(
            narrow_union_by_property_presence(&union, "a", true).unwrap(),
            named
        );
        assert_eq!(
            narrow_union_by_property_presence(&union, "a", false).unwrap(),
            other
        );
    }

    #[test]
    fn property_presence_flattens_an_alias_to_a_union() {
        let o1 = object(&[("in", false), ("key", false)]);
        let o2 = object(&[("key", false), ("map", false)]);
        let field = named_reference("Field", union_type(vec![o1.clone(), o2.clone()]));
        let fields = named_reference("Fields", object(&[("fields", false)]));
        let union = union_type(vec![field.clone(), fields.clone()]);

        assert_eq!(
            narrow_union_by_property_presence(&union, "in", true).unwrap(),
            o1
        );
        assert_eq!(
            narrow_union_by_property_presence(&union, "in", false).unwrap(),
            union_type(vec![o2, fields])
        );
        // Every constituent of `Field` has `key`, so the nominal wrapper survives
        // intact rather than being spliced into its constituents.
        assert_eq!(
            narrow_union_by_property_presence(&union, "key", true).unwrap(),
            field
        );
    }

    #[test]
    fn property_presence_narrows_an_alias_to_a_union_at_the_top_level() {
        let o1 = object(&[("in", false)]);
        let o2 = object(&[("map", false)]);
        let field = named_reference("Field", union_type(vec![o1.clone(), o2.clone()]));

        assert_eq!(
            narrow_union_by_property_presence(&field, "map", true).unwrap(),
            o2
        );
        assert_eq!(
            narrow_union_by_property_presence(&field, "map", false).unwrap(),
            o1
        );
    }

    #[test]
    fn property_presence_retains_an_optional_member_in_the_false_branch() {
        // tsc keeps `{ a?: number }` in the else branch of `if ("a" in v)` — the
        // key may legitimately be absent at runtime.
        let optional = object(&[("a", true), ("z", false)]);
        let without = object(&[("b", false)]);
        let union = union_type(vec![optional.clone(), without.clone()]);

        assert_eq!(
            narrow_union_by_property_presence(&union, "a", true).unwrap(),
            optional
        );
        assert!(narrow_union_by_property_presence(&union, "a", false).is_none());
    }

    #[test]
    fn property_presence_keeps_undecidable_members() {
        // A string index signature may supply the key, and a primitive member
        // cannot be classified — neither branch may drop them.
        let indexed = Type::Object(surge_ts_types::ObjectType::new(
            surge_ts_types::PropertyMap::default(),
            Some(Type::Number),
        ));
        let union = union_type(vec![indexed.clone(), Type::String]);
        assert!(narrow_union_by_property_presence(&union, "a", true).is_none());
        assert!(narrow_union_by_property_presence(&union, "a", false).is_none());

        // A decidably-absent member alongside them still goes in the true branch.
        let with_absent = union_type(vec![indexed, Type::String, object(&[("b", false)])]);
        assert_eq!(
            narrow_union_by_property_presence(&with_absent, "a", true).unwrap(),
            union
        );
    }
}
