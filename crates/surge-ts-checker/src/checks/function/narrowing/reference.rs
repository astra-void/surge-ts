use std::sync::Arc;
use surge_ts_syntax::ParsedUnaryOperator;
use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator};
use surge_ts_types::{Type, TypeCopyReason, union_type, with_type_copy_reason};

use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};
use super::guards::*;
use super::narrow_to_instanceof_subclass;

/// A guard applied to one reference (`o.p`, `this.a.b`, or a bare identifier).
#[derive(Debug, Clone, Copy)]
pub(super) enum ReferenceGuard<'a> {
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
        /// The constructor's instance type, when it resolved. A subject that is
        /// not a union narrows *down* to it (`s: Base` -> `Sub`), which member
        /// filtering cannot express.
        instance: Option<&'a Type>,
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
    /// A user-defined type predicate applied to a property path
    /// (`ts.isStringLiteral(node.moduleSpecifier)`).
    Predicate {
        target: &'a Type,
        keep_matching: bool,
    },
    /// `o.p === "lit"` / `o.p !== 3` on the property itself. The discriminant
    /// narrowers filter the *base* union by the same test; this one narrows the
    /// property's own type, which is what a later read of `o.p` sees.
    LiteralEquality {
        literal: &'a ParsedExpression,
        keep_matching: bool,
    },
}

impl ReferenceGuard<'_> {
    /// Narrows one leaf — a property's type plus its `optional` flag, or a whole
    /// binding's type with `optional = false`. `None` leaves the leaf alone.
    pub(super) fn narrow_leaf(&self, ty: &Type, optional: bool) -> Option<(Type, bool)> {
        match self {
            Self::Truthy => {
                let narrowed = surge_ts_types::remove_definitely_falsy(ty);
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
                instance,
                keep_matching,
            } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed =
                    narrow_union_by_instanceof(&effective, ctor_name, *instance, *keep_matching)
                        .or_else(|| {
                            narrow_to_instanceof_subclass(&effective, *instance, *keep_matching)
                        })?;
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
            Self::Predicate {
                target,
                keep_matching,
            } => {
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed = narrow_by_predicate(&effective, target, *keep_matching)?;
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
            // The complement of a literal test keeps `undefined` (an absent
            // property is not the literal either), and an optional slot stores
            // that `undefined` in its flag, so it is moved back there.
            Self::LiteralEquality {
                literal,
                keep_matching,
            } => {
                let literal = literal_expression_value(literal)?;
                let effective = Self::effective_leaf_type(ty, optional);
                let narrowed = narrow_by_literal_equality(&effective, &literal, *keep_matching)?;
                let still_optional = optional && type_includes_undefined_member(&narrowed);
                let narrowed = match narrowed {
                    // Nothing but the absent case is left: `tag?: never`.
                    // `remove_undefined` answers the degradation sentinel for a
                    // bare `undefined`, which would make the whole object read
                    // as unresolved and silence every later check on it.
                    Type::Undefined if still_optional => Type::Never,
                    narrowed if still_optional => surge_ts_types::remove_undefined(&narrowed),
                    narrowed => narrowed,
                };
                (narrowed != *ty || still_optional != optional).then_some((narrowed, still_optional))
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
                // An optional slot narrows even when the kept members equal
                // its written type: the assignment proved it present.
                (optional || narrowed != *ty).then_some((narrowed, false))
            }
        }
    }

    /// An optional property's `undefined` lives in its `optional` flag, not its
    /// type; a guard has to see it put back to split on it correctly.
    pub(super) fn effective_leaf_type(ty: &Type, optional: bool) -> Type {
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
pub(super) fn reference_path(expression: &ParsedExpression) -> Option<(String, Vec<String>)> {
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
pub(super) fn narrow_property_path(ty: &Type, path: &[String], guard: ReferenceGuard<'_>) -> Option<Type> {
    let (head, rest) = path.split_first()?;

    match ty.peeled() {
        Type::Object(mut object_type) => {
            let mut existing = object_type.properties.get(head.as_str())?.clone();
            existing.ty = crate::checks::function::settle_lazy_read(existing.ty);
            let (narrowed_ty, narrowed_optional) = if rest.is_empty() {
                guard.narrow_leaf(&existing.ty, existing.optional)?
            } else {
                // A truthy test further down the chain proves this link is
                // present too — the same fact that drops a nullish union
                // member below, applied to an `optional` property. That holds
                // even when the leaf itself does not change (`a.b?.c` with a
                // `string` `c`), so an unchanged leaf is not "nothing narrows".
                let deeper = narrow_property_path(&existing.ty, rest, guard);
                let proves_present = existing.optional && matches!(guard, ReferenceGuard::Truthy);
                if deeper.is_none() && !proves_present {
                    return None;
                }
                (
                    deeper.unwrap_or_else(|| existing.ty.clone()),
                    existing.optional && !proves_present,
                )
            };
            let properties = Arc::make_mut(&mut object_type.properties);
            properties.insert(
                head.as_str().into(),
                surge_ts_types::ObjectProperty {
                    ty: narrowed_ty,
                    optional: narrowed_optional,
                    method: existing.method,
                    readonly: existing.readonly,
                },
            );
            Some(Type::Object(object_type))
        }
        Type::Union(union) => {
            let mut narrowed_any = false;
            let mut members: Vec<Type> = Vec::with_capacity(union.types().len());
            for member in union.types() {
                match narrow_property_path(member, path, guard) {
                    Some(narrowed) => {
                        narrowed_any = true;
                        members.push(narrowed);
                    }
                    // A truthy test of `base?.p` also proves `base` itself is
                    // not nullish — a nullish base makes the whole chain
                    // `undefined`, which is falsy. Keeping the nullish member
                    // left `opts?.t ? f(opts.t) : …` reading `opts.t` as
                    // `string | undefined`.
                    None if matches!(guard, ReferenceGuard::Truthy)
                        && matches!(member, Type::Undefined | Type::Void) =>
                    {
                        narrowed_any = true;
                    }
                    // The leaf narrowers answer `None` both for "nothing to
                    // narrow" and for "this member cannot satisfy the guard at
                    // all", and keeping the member was wrong in the second case.
                    None if property_path_is_impossible(member, path, guard) => {
                        narrowed_any = true;
                    }
                    None => members.push(member.clone()),
                }
            }
            (narrowed_any && !members.is_empty()).then(|| union_type(members))
        }
        _ => None,
    }
}

/// Whether `guard` can never hold for `member` at `path`, which makes the member
/// an impossible shape of the narrowed value rather than an unchanged one.
///
/// The AWS SDK's union-member pattern is what needs this: every member of
/// `Field` declares the other members' keys as `?: never`, so
/// `field.arrayValue !== undefined` leaves exactly one member possible. At such
/// a leaf `never | undefined` collapses to plain `undefined`, which has no union
/// to split, so [`ReferenceGuard::narrow_leaf`] answers `None` — and `None` is
/// also what an already-narrow leaf answers. Read as "keep it", the union never
/// narrowed and every later `field.arrayValue` read stayed optional.
///
/// Only the not-nullish direction is decided here. The matching direction keeps
/// such a member (it *is* the undefined one), and the truthy guard reaches its
/// own arm above.
fn property_path_is_impossible(member: &Type, path: &[String], guard: ReferenceGuard<'_>) -> bool {
    if !matches!(
        guard,
        ReferenceGuard::Nullish {
            keep_matching: false
        }
    ) {
        return false;
    }
    let Some(leaf) = property_path_leaf_type(member, path) else {
        return false;
    };
    match leaf {
        Type::Undefined | Type::Void | Type::Never => true,
        Type::Union(union) => union
            .types()
            .iter()
            .all(|member| matches!(member, Type::Undefined | Type::Void | Type::Never)),
        _ => false,
    }
}

/// The declared type at the end of `path`, with an optional slot's `undefined`
/// put back. `None` when any link is missing or is not an object.
pub(super) fn property_path_leaf_type(ty: &Type, path: &[String]) -> Option<Type> {
    let (head, rest) = path.split_first()?;
    let Type::Object(object_type) = ty.peeled() else {
        return None;
    };
    let property = object_type.properties.get(head.as_str())?;
    let property_ty = crate::checks::function::settle_lazy_read(property.ty.clone());
    let effective = ReferenceGuard::effective_leaf_type(&property_ty, property.optional);
    if rest.is_empty() {
        return Some(effective);
    }
    property_path_leaf_type(&property_ty, rest)
}

/// The narrowed type of `base` under `guard` applied at `path`, or `None` when
/// nothing changes. An empty `path` guards the binding itself.
pub(super) fn narrowed_reference_type(ty: &Type, path: &[String], guard: ReferenceGuard<'_>) -> Option<Type> {
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
pub(super) fn narrow_reference_in_scope(
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
pub(super) fn collect_reference_guards<'a>(
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
    // By De Morgan every operand of an `||` fails where the whole test does, so
    // `if (!s?.program || !s.map) return;` leaves both references truthy.
    if let ParsedExpression::Logical {
        left,
        operator: ParsedLogicalOperator::Or,
        right,
        ..
    } = condition
    {
        if !branch_is_true {
            collect_reference_guards(left, false, guards);
            collect_reference_guards(right, false, guards);
        }
        return;
    }

    // `!!x` is `x`'s truthiness spelled out; as an `&&` operand (`!!query &&
    // query.isFetched()`, or an alias of it) it proves the same thing the bare
    // reference does. Unwrapped here so every guard below sees the reference.
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
        && let ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand: inner,
            ..
        } = operand.as_ref()
    {
        collect_reference_guards(inner, branch_is_true, guards);
        return;
    }
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        collect_reference_guards(operand, !branch_is_true, guards);
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
                    instance: None,
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

    if let Some((reference, literal, eq)) = parse_reference_literal_equality(condition) {
        if let Some((base, path)) = reference_path(reference)
            && !path.is_empty()
        {
            guards.push((
                base,
                path,
                ReferenceGuard::LiteralEquality {
                    literal,
                    keep_matching: branch_is_true == eq,
                },
            ));
        }
        return;
    }

    if branch_is_true && let Some((base, path)) = reference_path(condition) {
        guards.push((base, path, ReferenceGuard::Truthy));
    }
}

pub(super) fn type_includes_undefined_member(ty: &Type) -> bool {
    match ty {
        Type::Undefined => true,
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| matches!(member, Type::Undefined)),
        _ => false,
    }
}

/// Narrows `symbols` by a truthy or `typeof` guard on a reference the
/// identifier-keyed guards above do not reach — `o.p ? o.p : …`,
/// `typeof o.p === "string" ? o.p : …`. Returns `None` when nothing narrows.
pub(super) fn narrow_reference_guard_symbol_table(
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

/// Applies `o.p === "lit"` / `o?.p !== 3` narrowing to the property `o.p` in
/// place. Returns whether a reference test was recognised (not whether it
/// narrowed): the caller keeps going either way.
pub(super) fn narrow_literal_equality_reference_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((reference, literal, eq)) = parse_reference_literal_equality(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(reference) else {
        return false;
    };
    if path.is_empty() {
        return false;
    }
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::LiteralEquality {
            literal,
            keep_matching: branch_is_true == eq,
        },
        scopes,
    );
    true
}
