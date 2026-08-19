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
                        method: existing.method,
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

/// A guard applied to one reference (`o.p`, `this.a.b`, or a bare identifier).
#[derive(Debug, Clone, Copy)]
enum ReferenceGuard<'a> {
    Truthy,
    Typeof {
        tag: &'a str,
        keep_matching: bool,
    },
    PropertyPresence {
        property: &'a str,
        keep_present: bool,
    },
    Instanceof {
        ctor_name: &'a str,
        keep_matching: bool,
    },
    Arrayness {
        keep_arrays: bool,
    },
    ArrayBufferView {
        keep_views: bool,
    },
    Nullish {
        keep_matching: bool,
    },
    Assigned {
        assigned: &'a Type,
    },
}

impl ReferenceGuard<'_> {
    /// Narrows one leaf — a property's type plus its `optional` flag, or a whole
    /// binding's type with `optional = false`. `None` leaves the leaf alone.
    fn narrow_leaf(&self, ty: &Type, optional: bool) -> Option<(Type, bool)> {
        match self {
            Self::Truthy => {
                let narrowed = surge_ts_types::remove_nullish(ty);
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            // An optional property carries its `undefined` in the `optional` flag
            // rather than the type, so the tag test has to see it put back.
            Self::Typeof { tag, keep_matching } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed = narrow_union_by_typeof(&effective, tag, *keep_matching)?;
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            Self::PropertyPresence {
                property,
                keep_present,
            } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed =
                    narrow_union_by_property_presence(&effective, property, *keep_present)?;
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            Self::Instanceof {
                ctor_name,
                keep_matching,
            } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed = narrow_union_by_instanceof(&effective, ctor_name, *keep_matching)?;
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            Self::Arrayness { keep_arrays } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed = narrow_union_by_arrayness(&effective, *keep_arrays)?;
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            Self::ArrayBufferView { keep_views } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed = narrow_union_by_arraybufferview(&effective, *keep_views)?;
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            Self::Nullish { keep_matching } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed = narrow_union_by_nullish(&effective, *keep_matching)?;
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            // An assignment narrows a union-declared slot to the members the
            // assigned value can inhabit, as tsc does. A non-union slot is
            // already as precise as the declaration allows, so it is left alone.
            Self::Assigned { assigned } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let Type::Union(union) = &effective else {
                    return None;
                };
                // Same scan-before-clone rule as the truthiness split: an
                // assignment that rules nothing out must not rebuild the union.
                let kept_count = union
                    .types()
                    .iter()
                    .filter(|member| surge_ts_types::is_assignable_to(assigned, member))
                    .count();
                if kept_count == 0 || kept_count == union.types().len() {
                    return None;
                }
                let narrowed = union_type(
                    union
                        .types()
                        .iter()
                        .filter(|member| surge_ts_types::is_assignable_to(assigned, member))
                        .cloned()
                        .collect(),
                );
                (narrowed != *ty).then_some((narrowed, false))
            }
        }
    }

    /// An optional property's `undefined` lives in its `optional` flag, not its
    /// type; a guard has to see it put back to split on it correctly.
    fn effective_leaf_type(ty: &Type, optional: bool) -> Type {
        if optional {
            union_type(vec![ty.clone(), Type::Undefined])
        } else {
            ty.clone()
        }
    }
}

/// The base symbol name and property chain of a plain reference expression:
/// `x` → `("x", [])`, `this.a.b` → `("this", ["a", "b"])`. `None` for anything
/// that is not a static reference (a call, a computed index, a literal).
fn reference_path(expression: &ParsedExpression) -> Option<(String, Vec<String>)> {
    match expression {
        ParsedExpression::Identifier { name, .. } => Some((name.clone(), Vec::new())),
        ParsedExpression::This { .. } => Some(("this".to_string(), Vec::new())),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            ..
        } => {
            let (base, mut path) = reference_path(object)?;
            path.push(property_name.clone());
            Some((base, path))
        }
        ParsedExpression::NonNullAssertion { expression, .. } => reference_path(expression),
        _ => None,
    }
}

/// Rebuilds `ty` with the property reached by `path` replaced by the guard's
/// narrowing, distributing over unions and peeling named references. `None` when
/// the path does not exist or nothing narrows, so the caller leaves the binding
/// untouched.
fn narrow_property_path(ty: &Type, path: &[String], guard: ReferenceGuard<'_>) -> Option<Type> {
    let (head, rest) = path.split_first()?;

    match ty.peeled() {
        Type::Object(mut object_type) => {
            let existing = object_type.properties.get(head.as_str())?.clone();
            let (narrowed_ty, narrowed_optional) = if rest.is_empty() {
                guard.narrow_leaf(&existing.ty, existing.optional)?
            } else {
                (
                    narrow_property_path(&existing.ty, rest, guard)?,
                    existing.optional,
                )
            };
            let properties = Arc::make_mut(&mut object_type.properties);
            properties.insert(
                head.as_str().into(),
                surge_ts_types::ObjectProperty {
                    ty: narrowed_ty,
                    optional: narrowed_optional,
                    method: false,
                },
            );
            Some(Type::Object(object_type))
        }
        Type::Union(union) => {
            let mut narrowed_any = false;
            let members: Vec<Type> = union
                .types()
                .iter()
                .map(|member| match narrow_property_path(member, path, guard) {
                    Some(narrowed) => {
                        narrowed_any = true;
                        narrowed
                    }
                    None => member.clone(),
                })
                .collect();
            narrowed_any.then(|| union_type(members))
        }
        _ => None,
    }
}

/// The narrowed type of `base` under `guard` applied at `path`, or `None` when
/// nothing changes. An empty `path` guards the binding itself.
fn narrowed_reference_type(ty: &Type, path: &[String], guard: ReferenceGuard<'_>) -> Option<Type> {
    let narrowed = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        if path.is_empty() {
            guard.narrow_leaf(ty, false).map(|(narrowed, _)| narrowed)
        } else {
            narrow_property_path(ty, path, guard)
        }
    })?;
    (narrowed != *ty).then_some(narrowed)
}

/// Narrows the reference `target` (an `o.p` member expression) to what an
/// assignment of `assigned` leaves it able to be. Block-scoped like every other
/// in-scope narrowing, so a branch's assignment does not leak past `pop_child`.
pub(crate) fn narrow_assignment_target_in_scope(
    target: &ParsedExpression,
    assigned: &Type,
    scopes: &mut ScopeStack,
) {
    let Some((base, path)) = reference_path(target) else {
        return;
    };
    if path.is_empty() {
        return;
    }
    narrow_reference_in_scope(&base, &path, ReferenceGuard::Assigned { assigned }, scopes);
}

/// Applies `guard` to `base`'s `path` in the *current* scope frame — see the
/// shadowing note in [`narrow_discriminant_in_scope`].
fn narrow_reference_in_scope(
    base: &str,
    path: &[String],
    guard: ReferenceGuard<'_>,
    scopes: &mut ScopeStack,
) {
    let Some(symbol) = scopes.resolve(base) else {
        return;
    };
    let Some(narrowed) = narrowed_reference_type(&symbol.ty, path, guard) else {
        return;
    };
    let declared = symbol.ty.clone();
    let narrowed_symbol = SymbolInfo {
        ty: narrowed,
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let _ = scopes.insert_current_narrowed(base.to_string(), narrowed_symbol, declared);
}

/// The reference guards a condition establishes for `branch_is_true`: a `typeof`
/// test on a reference, or every reference an `&&` chain proves truthy. Only
/// truthiness of the true branch is modelled — the falsy complement (`""`, `0`,
/// …) is not.
fn collect_reference_guards<'a>(
    condition: &'a ParsedExpression,
    branch_is_true: bool,
    guards: &mut Vec<(String, Vec<String>, ReferenceGuard<'a>)>,
) {
    if let ParsedExpression::Logical {
        left,
        operator: ParsedLogicalOperator::And,
        right,
        ..
    } = condition
    {
        if branch_is_true {
            collect_reference_guards(left, true, guards);
            collect_reference_guards(right, true, guards);
        }
        return;
    }

    if let Some((operand, tag, eq)) = parse_typeof_condition(condition) {
        if let Some((base, path)) = reference_path(operand) {
            guards.push((
                base,
                path,
                ReferenceGuard::Typeof {
                    tag,
                    keep_matching: branch_is_true == eq,
                },
            ));
        }
        return;
    }

    if let Some((operand, ctor_name)) = parse_instanceof_condition(condition) {
        if let Some((base, path)) = reference_path(operand) {
            guards.push((
                base,
                path,
                ReferenceGuard::Instanceof {
                    ctor_name,
                    keep_matching: branch_is_true,
                },
            ));
        }
        return;
    }

    if let Some(operand) = parse_array_isarray_condition(condition) {
        if let Some((base, path)) = reference_path(operand) {
            guards.push((
                base,
                path,
                ReferenceGuard::Arrayness {
                    keep_arrays: branch_is_true,
                },
            ));
        }
        return;
    }

    if let Some(operand) = parse_arraybuffer_isview_condition(condition) {
        if let Some((base, path)) = reference_path(operand) {
            guards.push((
                base,
                path,
                ReferenceGuard::ArrayBufferView {
                    keep_views: branch_is_true,
                },
            ));
        }
        return;
    }

    if let Some((subject, eq)) = parse_nullish_equality_condition(condition) {
        if let Some((base, path)) = reference_path(subject) {
            guards.push((
                base,
                path,
                ReferenceGuard::Nullish {
                    keep_matching: branch_is_true == eq,
                },
            ));
        }
        return;
    }

    if let Some((object, property)) = parse_in_condition(condition) {
        if let Some((base, path)) = reference_path(object) {
            guards.push((
                base,
                path,
                ReferenceGuard::PropertyPresence {
                    property,
                    keep_present: branch_is_true,
                },
            ));
        }
        return;
    }

    if branch_is_true && let Some((base, path)) = reference_path(condition) {
        guards.push((base, path, ReferenceGuard::Truthy));
    }
}

/// Resolves a predicate guard's target type under the predicate's declaring
/// file (see [`crate::symbols::FunctionSignatureInfo::declaring_file`]). A
/// resolution that degrades (`had_error` or the `Unknown` sentinel) proves
/// nothing — narrowing on it would manufacture facts from a modeling gap — so
/// it yields `None`.
fn resolve_predicate_guard_type(
    guard: &PredicateGuardInfo,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let declaring_file = guard
        .declaring_file
        .as_deref()
        .filter(|file| *file != ctx.file_name)
        .map(str::to_string);
    let saved_file_name = declaring_file.map(|file| {
        let saved = ctx.file_name.clone();
        ctx.set_file_name(file);
        saved
    });
    let resolved = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        crate::infer::types::resolve_parsed_type(
            guard.predicate_type.clone(),
            ctx,
            &mut Vec::new(),
            &crate::infer::TypeParameterSubstitution::new(),
        )
    });
    if let Some(saved) = saved_file_name {
        ctx.set_file_name(saved);
    }
    if resolved.had_error() {
        return None;
    }
    let ty = resolved.into_ty();
    (!matches!(ty, Type::Unknown)).then_some(ty)
}

/// Applies user-defined type-predicate narrowing (`isFoo(x)`) in place to a
/// `ScopeStack`. Returns whether the condition was such a predicate call over a
/// bare-identifier argument.
fn narrow_predicate_call_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> bool {
    let Some(guard) = parse_type_predicate_condition(condition, &mut |name| {
        scopes
            .resolve(name)
            .and_then(|symbol| symbol.function_signature.clone())
    }) else {
        return false;
    };
    let Some(symbol) = scopes.resolve(&guard.subject) else {
        return true;
    };
    let subject_ty = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();
    let Some(predicate_ty) = resolve_predicate_guard_type(&guard, ctx) else {
        return true;
    };
    let Some(narrowed) = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        narrow_by_predicate(&subject_ty, &predicate_ty, branch_is_true)
    }) else {
        return true;
    };
    let _ = scopes.insert_current_narrowed(
        guard.subject,
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        subject_ty,
    );
    true
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
    scopes: &ScopeStack,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => narrow_type_for_identifier(operand, var_name, ty, !branch_is_true, scopes, ctx),
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

/// Narrows `ty` for `var_name` under a single (non-composite) guard condition.
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
    if let Some(guard) = parse_type_predicate_condition(condition, &mut |name| {
        scopes
            .resolve(name)
            .and_then(|symbol| symbol.function_signature.clone())
    }) && guard.subject == var_name
    {
        let predicate_ty = resolve_predicate_guard_type(&guard, ctx)?;
        return narrow_by_predicate(ty, &predicate_ty, branch_is_true);
    }
    if let Some((ParsedExpression::Identifier { name, .. }, property, literal, eq)) =
        parse_discriminant_condition(condition)
        && name == var_name
    {
        return narrow_union_by_discriminant(ty, property, &literal, branch_is_true == eq);
    }
    if let Some((ParsedExpression::Identifier { name, .. }, eq)) =
        parse_nullish_equality_condition(condition)
        && name == var_name
    {
        return narrow_union_by_nullish(ty, branch_is_true == eq);
    }
    if let Some((ParsedExpression::Identifier { name, .. }, property)) =
        parse_in_condition(condition)
        && name == var_name
    {
        return narrow_union_by_property_presence(ty, property, branch_is_true);
    }
    None
}

/// Collects the identifiers tested by equality guards — a discriminant test
/// (`x.kind === "a"` → `x`) or a nullish test (`x === null`) — within a
/// (possibly `||`/`&&`/`!`-composed) condition. Kept out of
/// [`collect_guard_operand_identifiers`], whose results also drive the
/// genuine-`unknown` downgrade — testing a *property* proves nothing about the
/// whole value there.
fn collect_equality_guard_subjects(condition: &ParsedExpression, names: &mut Vec<String>) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_equality_guard_subjects(operand, names),
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or | ParsedLogicalOperator::And,
            right,
            ..
        } => {
            collect_equality_guard_subjects(left, names);
            collect_equality_guard_subjects(right, names);
        }
        _ => {
            let subject = match parse_discriminant_condition(condition) {
                Some((ParsedExpression::Identifier { name, .. }, _, _, _)) => Some(name),
                _ => match parse_nullish_equality_condition(condition) {
                    Some((ParsedExpression::Identifier { name, .. }, _)) => Some(name),
                    _ => None,
                },
            };
            if let Some(name) = subject
                && !names.iter().any(|existing| existing == name)
            {
                names.push(name.clone());
            }
        }
    }
}

/// Collects the subjects of user-defined predicate guard calls (`isFoo(x)` → `x`)
/// within a (possibly `||`/`&&`/`!`-composed) condition. Kept separate from
/// [`collect_guard_operand_identifiers`]: recognizing a predicate call needs the
/// callee's collected signature, and a non-predicate call must not count as a
/// guard (tsc does not narrow `if (foo(x))`).
fn collect_predicate_guard_subjects(
    condition: &ParsedExpression,
    scopes: &ScopeStack,
    names: &mut Vec<String>,
) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_predicate_guard_subjects(operand, scopes, names),
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or | ParsedLogicalOperator::And,
            right,
            ..
        } => {
            collect_predicate_guard_subjects(left, scopes, names);
            collect_predicate_guard_subjects(right, scopes, names);
        }
        _ => {
            if let Some(guard) = parse_type_predicate_condition(condition, &mut |name| {
                scopes
                    .resolve(name)
                    .and_then(|symbol| symbol.function_signature.clone())
            }) && !names.iter().any(|existing| existing == &guard.subject)
            {
                names.push(guard.subject);
            }
        }
    }
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
    collect_predicate_guard_subjects(condition, scopes, &mut operand_names);
    collect_equality_guard_subjects(condition, &mut operand_names);

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
    // `!guard` narrows the opposite branch.
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        return narrow_condition_symbol_table(operand, symbols, !branch_is_true);
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

    narrow_discriminant_symbol_table(condition, symbols, branch_is_true)
        .or_else(|| narrow_typeof_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_instanceof_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_array_isarray_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_property_presence_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_nullish_equality_symbol_table(condition, symbols, branch_is_true))
        .or_else(|| narrow_reference_guard_symbol_table(condition, symbols, branch_is_true))
}

/// Narrows `symbols` by a truthy or `typeof` guard on a reference the
/// identifier-keyed guards above do not reach — `o.p ? o.p : …`,
/// `typeof o.p === "string" ? o.p : …`. Returns `None` when nothing narrows.
fn narrow_reference_guard_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let mut guards = Vec::new();
    collect_reference_guards(condition, branch_is_true, &mut guards);
    if guards.is_empty() {
        return None;
    }

    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    let mut changed = false;
    for (base, path, guard) in guards {
        let Some(symbol) = narrowed_symbols.get(&base) else {
            continue;
        };
        let Some(narrowed) = narrowed_reference_type(&symbol.ty, &path, guard) else {
            continue;
        };
        let declared = symbol.ty.clone();
        let narrowed_symbol = SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        };
        narrowed_symbols.insert_narrowed(base, narrowed_symbol, declared);
        changed = true;
    }

    changed.then_some(narrowed_symbols)
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
        let declared = symbol.ty.clone();
        let narrowed_symbol = SymbolInfo {
            ty: new_ty,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        };
        narrowed.insert_narrowed(base_name.clone(), narrowed_symbol, declared);
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
    ctx: &mut CheckerContext,
) {
    // `!guard` narrows the opposite branch.
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        narrow_discriminant_in_scope(operand, scopes, !branch_is_true, ctx);
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

    if narrow_logical_guard_in_scope(condition, scopes, branch_is_true, ctx) {
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
    if narrow_property_presence_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_predicate_call_in_scope(condition, scopes, branch_is_true, ctx) {
        return;
    }
    if narrow_truthy_reference_in_scope(condition, scopes, branch_is_true) {
        return;
    }
    if narrow_nullish_equality_in_scope(condition, scopes, branch_is_true) {
        return;
    }

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

/// Positive truthy narrowing of a reference guard (`if (x) { … }`,
/// `if (o.p) { … }`, `if (this.p) { … }`): the true branch drops the nullish
/// members (`undefined`/`void`) the truthy test excludes, so a `T | undefined`
/// callee/value/property resolves to `T` inside the block. Only the true branch
/// narrows; the falsy complement (`""`, `0`, `false`, …) is not modelled, so the
/// false branch keeps the original type. The `!guard` unwrap in
/// [`narrow_discriminant_in_scope`] routes `if (!x)` else/fall-through here with
/// `branch_is_true` already flipped. Returns whether the condition was such a
/// reference (always handled, so equality-discriminant parsing is skipped).
fn narrow_truthy_reference_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    // `if (a?.b())` proves `a` non-nullish in the true branch: had it been
    // nullish the whole chain would short-circuit to `undefined` and the test
    // would be false. `reference_path` stops at the call, so unwrap to its
    // receiver first.
    let condition = match condition {
        ParsedExpression::OptionalPropertyCall { object, .. }
        | ParsedExpression::OptionalCall { callee: object, .. } => object.as_ref(),
        other => other,
    };
    let Some((base, path)) = reference_path(condition) else {
        return false;
    };
    // `if (!merged.valid) throw` discriminates a union by a boolean *literal*
    // member, which the leaf-level truthy guard cannot express: it narrows the
    // property in place instead of dropping the members the test rules out.
    narrow_union_by_property_truthiness_in_scope(&base, &path, branch_is_true, scopes);
    if branch_is_true {
        narrow_reference_in_scope(&base, &path, ReferenceGuard::Truthy, scopes);
    }
    true
}

/// Whether the type at `path` inside `member` is always truthy (`Some(true)`),
/// always falsy (`Some(false)`), or undecidable (`None`). Only unit types decide;
/// everything else stays in both branches.
fn path_truthiness(member: &Type, path: &[String]) -> Option<bool> {
    let mut current = member.peeled();
    for segment in path {
        let Type::Object(object) = &current else {
            return None;
        };
        let property = object.properties.get(segment.as_str())?;
        if property.is_optional() {
            return None;
        }
        current = property.ty.peeled();
    }
    match &current {
        Type::BooleanLiteral(value) => Some(*value),
        Type::StringLiteral(value) => Some(!value.is_empty()),
        Type::NumberLiteral(literal) => Some(literal.value.parse::<f64>().ok()? != 0.0),
        Type::Undefined | Type::Void | Type::Never => Some(false),
        _ => None,
    }
}

/// Drops the union members a truthiness test on `path` rules out. Leaves a
/// non-union base, and any member whose leaf is not a unit type, untouched.
fn narrow_union_by_property_truthiness_in_scope(
    base: &str,
    path: &[String],
    branch_is_true: bool,
    scopes: &mut ScopeStack,
) {
    if path.is_empty() {
        return;
    }
    let Some(symbol) = scopes.resolve(base) else {
        return;
    };
    let peeled = symbol.ty.peeled();
    let Type::Union(union) = &peeled else {
        return;
    };
    // Decide before cloning: the overwhelmingly common case is a union the test
    // does not partition at all, and building the kept vector first paid a full
    // member clone for every one of them.
    let dropped = union
        .types()
        .iter()
        .filter(|member| path_truthiness(member, path) == Some(!branch_is_true))
        .count();
    if dropped == 0 || dropped == union.types().len() {
        return;
    }
    let kept: Vec<Type> = union
        .types()
        .iter()
        .filter(|member| path_truthiness(member, path) != Some(!branch_is_true))
        .cloned()
        .collect();
    let declared = symbol.ty.clone();
    let narrowed_symbol = SymbolInfo {
        ty: union_type(kept),
        kind: symbol.kind,
        function_signature: symbol.function_signature.clone(),
    };
    let _ = scopes.insert_current_narrowed(base.to_string(), narrowed_symbol, declared);
}

/// Applies `typeof x === "tag"` / `typeof o.p === "tag"` narrowing in place to a
/// `ScopeStack` (the if-body and early-return paths). Returns whether the
/// condition was a typeof guard over a reference.
fn narrow_typeof_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((operand, tag, eq)) = parse_typeof_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    let guard = ReferenceGuard::Typeof {
        tag,
        keep_matching: branch_is_true == eq,
    };
    narrow_reference_in_scope(&base, &path, guard, scopes);
    true
}

/// Applies `x instanceof Ctor` / `o.p instanceof Ctor` narrowing in place to a
/// `ScopeStack`. Returns whether the condition was an instanceof guard over a
/// reference.
fn narrow_instanceof_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((operand, ctor_name)) = parse_instanceof_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::Instanceof {
            ctor_name,
            keep_matching: branch_is_true,
        },
        scopes,
    );
    true
}

/// Applies `Array.isArray(x)` / `Array.isArray(o.p)` narrowing in place to a
/// `ScopeStack`. Returns whether the condition was such a guard over a
/// reference.
fn narrow_array_isarray_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some(operand) = parse_array_isarray_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::Arrayness {
            keep_arrays: branch_is_true,
        },
        scopes,
    );
    true
}

/// Applies `"prop" in x` narrowing in place to a `ScopeStack` (the if-body and
/// early-return paths). Returns whether the condition was such a test over a
/// bare identifier.
fn narrow_property_presence_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((object, property)) = parse_in_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(object) else {
        return false;
    };
    // Shadows in the current frame, not the owning frame — see the note in
    // `narrow_discriminant_in_scope`.
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::PropertyPresence {
            property,
            keep_present: branch_is_true,
        },
        scopes,
    );
    true
}

/// Applies `x === null` / `x.p === undefined` narrowing in place to a
/// `ScopeStack`, for a bare identifier or one property of one. Returns whether
/// the condition was such a test (handled either way, so the discriminant parse
/// downstream is skipped — `null` is not a discriminant literal).
fn narrow_nullish_equality_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((subject, eq)) = parse_nullish_equality_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(subject) else {
        return false;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::Nullish {
            keep_matching: branch_is_true == eq,
        },
        scopes,
    );
    true
}

/// Applies `ArrayBuffer.isView(x)` / `ArrayBuffer.isView(o.p)` narrowing in place
/// to a `ScopeStack`. Returns whether the condition was such a guard over a
/// reference.
fn narrow_arraybuffer_isview_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some(operand) = parse_arraybuffer_isview_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::ArrayBufferView {
            keep_views: branch_is_true,
        },
        scopes,
    );
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

        let declared = symbol.ty.clone();
        let narrowed_symbol = SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        };
        narrowed_symbols.insert_narrowed(base_name.clone(), narrowed_symbol, declared);
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
