use super::*;

use surge_ts_syntax::{ParsedConditionalType, ParsedFunctionType, ParsedNamedType};
use surge_ts_types::is_assignable_to;

use crate::symbols::TypeDeclarationInfo;

/// Evaluates a narrow subset of conditional types `Check extends Extends ? True
/// : False`.
///
/// Two shapes are supported:
/// - **Distributive**: when the check type is a naked type parameter (a `Named`
///   reference that the current substitution has bound to a concrete type), the
///   conditional distributes over each member of the substituted union. This is
///   what backs `Exclude`, `Extract`, and `NonNullable`.
/// - **Concrete**: when the check type is not a naked parameter but resolves to a
///   concrete type, a single assignability test selects the branch.
///
/// Anything outside this subset (an unresolved generic check type, or a branch
/// that already failed to resolve) degrades to `Unknown` so callers do not
/// cascade.
pub(crate) fn resolve_conditional_type(
    conditional: ParsedConditionalType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let distributive_parameter = match conditional.check_type.as_ref() {
        ParsedType::Named(named) => substitution
            .get(&named.name)
            .filter(|_| !substitution.is_placeholder(&named.name))
            .map(|_| named.name.clone()),
        _ => None,
    };

    // Kept for `infer X` binding: the parsed pattern is matched structurally
    // against the resolved check type on the true branch so captures like
    // `T` in `S extends Box<infer T> ? T : never` resolve to the real argument.
    let extends_pattern = (*conditional.extends_type).clone();

    // Two generic signatures whose returns are conditionals over their own type
    // parameter are related by *identity*, not assignability — tsc cannot evaluate
    // either conditional with the parameter unbound, so it compares them
    // structurally. That is the entire mechanism behind the `Equal<a, b>` idiom
    // (`(<T>() => T extends a ? 1 : 2) extends (<T>() => T extends b ? 1 : 2)`),
    // which every type-level test suite is written with. surge resolves both
    // returns to the same `unknown` sentinel, so the plain assignability test
    // below answers `true` for every pair of types.
    match deferred_conditional_identity(
        &conditional.check_type,
        &extends_pattern,
        ctx,
        resolving,
        substitution,
    ) {
        DeferredIdentity::Identical(identical) => {
            let branch = if identical {
                (*conditional.true_type).clone()
            } else {
                (*conditional.false_type).clone()
            };
            return resolve_parsed_type(branch, ctx, resolving, substitution);
        }
        // The shape is an identity test but one side is a modelling gap: falling
        // through to the assignability test below would answer `true` (both
        // deferred returns are the same sentinel), which is how every
        // `Equal<Pattern<input>, p>` guard picked its `never` arm.
        DeferredIdentity::Undecidable => {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: false,
            };
        }
        DeferredIdentity::NotThisShape => {}
    }

    let resolved_extends =
        resolve_parsed_type(*conditional.extends_type, ctx, resolving, substitution);
    // Only bail when the extends pattern is structureless: a usable shape that
    // merely tainted `had_error` from an unmodelled deep member (e.g. React's
    // `JSXElementConstructor<P>`, whose body pulls `ReactNode`/`Component`) is
    // still enough to decide the branch assignability test, so the conditional
    // must proceed rather than collapse — that collapse is what blocked
    // `ComponentProps<"input">` from selecting its `JSX.IntrinsicElements[T]` branch.
    if resolved_extends.had_error && resolved_extends.ty.is_unknown() {
        if crate::infer::types::interface::had_error_trace_enabled() {
            eprintln!(
                "[had-error] conditional-extends cp={} in file {}",
                crate::program::in_check_phase(),
                ctx.file_name
            );
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    let resolved_check = resolve_parsed_type(
        (*conditional.check_type).clone(),
        ctx,
        resolving,
        substitution,
    );
    if resolved_check.had_error {
        if crate::infer::types::interface::had_error_trace_enabled() {
            eprintln!(
                "[had-error] conditional-check cp={} in file {}",
                crate::program::in_check_phase(),
                ctx.file_name
            );
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    if let Some(parameter_name) = distributive_parameter {
        // A deferred alias instantiation can carry its union behind a lazy
        // nominal reference; distribution must see the structural union (tsc
        // distributes `A | B` into `F<A> | F<B>`), so peel a reference check
        // type before matching. A non-union peel keeps the original reference
        // as the single member, preserving its nominal fast paths in the
        // branch assignability test.
        let peeled_check;
        let distribution_shape = match &resolved_check.ty {
            Type::Reference(_) => {
                peeled_check = crate::program::with_dts_expansion_reason(
                    crate::program::DtsExpansionReason::ConditionalType,
                    || resolved_check.ty.peeled(),
                );
                &peeled_check
            }
            other => other,
        };
        let members = match distribution_shape {
            Type::Union(union) => union.types().to_vec(),
            Type::Never => Vec::new(),
            _ => vec![resolved_check.ty.clone()],
        };

        let _expansion_scope = TypeExpansionScope::enter();
        let mut results = Vec::new();
        let mut had_error = false;
        for member in members {
            if !try_consume_type_expansion_step() {
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: false,
                };
            }
            // Same "cannot decide" degrade as the non-distributive path below: a
            // member that collapsed to the `unknown` sentinel (a value type surge
            // could not model, e.g. `ComponentProps<typeof UnmodelledValue>`) must
            // not deterministically select a branch — the false branch would
            // produce a closed concrete shape (`{}`) and flag every real property
            // as excess. The genuine `unknown` keyword is `GenuineUnknown` and
            // still evaluates normally.
            if matches!(member, Type::Unknown | Type::TypeParameter(_)) {
                results.push(Type::Unknown);
                continue;
            }
            // A genuine `unknown` member decides its branch like any other type,
            // but only against a constraint surge actually modelled. With an
            // unmodelled one the loop below falls to the false branch, and a
            // declaration whose false arm is `never` (`R extends Record<string,
            // unknown> ? R : never`) then hands back `never` — a concrete wrong
            // answer built on a gap, rather than a degrade.
            if matches!(member, Type::GenuineUnknown) && resolved_extends.ty.is_unknown() {
                results.push(Type::Unknown);
                continue;
            }
            // Same `any` rule as the non-distributive path below: an `any`
            // member makes the branch indeterminate (tsc yields the union of
            // both branches), so degrade to an open `any` rather than selecting
            // the true branch — which would also leave its `infer` captures
            // unbound (`MakeReadonly<any>` was measured resolving
            // `ReadonlyMap<K, V>` with `K`/`V` unresolvable at thousands of zod
            // sites, silently degrading every enclosing interface expansion).
            if matches!(member, Type::Any) {
                results.push(Type::Any);
                continue;
            }
            // The sentinel guard above only sees a *syntactic* `unknown`; a
            // member carried behind a lazy reference (`output<T>` whose body
            // could not resolve) peels to the same sentinel and must get the
            // same "cannot decide" treatment instead of matching the branch
            // test (`unknown` is assignable to everything, so it would always
            // select the true branch with its captures unbound).
            if matches!(member, Type::Reference(_)) {
                let peeled = crate::program::with_dts_expansion_reason(
                    crate::program::DtsExpansionReason::ConditionalType,
                    || member.peeled(),
                );
                if matches!(peeled, Type::Unknown | Type::TypeParameter(_)) {
                    results.push(Type::Unknown);
                    continue;
                }
                if matches!(peeled, Type::Any) {
                    results.push(Type::Any);
                    continue;
                }
            }
            let mut member_substitution =
                substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
            member_substitution.insert(parameter_name.clone(), member.clone());

            // An extends pattern surge could not model (`unknown`) must not be
            // treated as a matched constraint: `is_assignable_to(x, unknown)` is
            // always true (unknown is the top type), which would pick the true
            // branch for every member. tsc keeps the constraint meaningful, so an
            // unmodelled extends falls to the false branch instead — this is what
            // lets `ComponentProps<"input">` skip its `JSXElementConstructor<infer>`
            // branch (whose body resolves to `unknown` here) and reach the
            // `keyof JSX.IntrinsicElements` branch.
            // `T extends unknown ? … : …` is how a declaration forces distribution,
            // and `unknown` is the top type: every member satisfies it. The guard
            // below rejects an *unmodelled* extends clause, and the degradation
            // sentinel shares `is_unknown()` with the genuine keyword, so without
            // this the true branch was unreachable and the false arm — `never` in
            // the `UnionToIntersection` spellings that use it — always won.
            let extends_is_top = matches!(resolved_extends.ty, Type::GenuineUnknown);
            let branch = match try_tuple_infer_match(
                &extends_pattern,
                &member,
                &member_substitution,
                ctx,
                resolving,
            ) {
                TuplePatternMatch::Matched(matched) => {
                    member_substitution = matched;
                    (*conditional.true_type).clone()
                }
                TuplePatternMatch::Rejected => (*conditional.false_type).clone(),
                TuplePatternMatch::Undecided => {
                    // A pattern with captures whose own shape degraded (an
                    // interface instantiated over placeholders that lost a
                    // member) is permissive for the wrong reason: the
                    // assignability test would take the true branch for a
                    // member that matches nothing, binding nothing.
                    if pattern_shape_degraded(&extends_pattern, &resolved_extends) {
                        results.push(Type::Unknown);
                        continue;
                    }
                    if extends_is_top
                        || (!resolved_extends.ty.is_unknown()
                            && is_assignable_to(&member, &resolved_extends.ty))
                    {
                        seed_infer_placeholders(&extends_pattern, &mut member_substitution);
                        bind_infer_captures(
                            &extends_pattern,
                            &member,
                            &mut member_substitution,
                            ctx,
                            resolving,
                            0,
                            true,
                        );
                        (*conditional.true_type).clone()
                    } else if let Some(matched) = try_function_infer_match(
                        &extends_pattern,
                        &member,
                        &member_substitution,
                        ctx,
                        resolving,
                    ) {
                        member_substitution = matched;
                        (*conditional.true_type).clone()
                    } else {
                        (*conditional.false_type).clone()
                    }
                }
            };

            if crate::infer::types::interface::had_error_trace_enabled() {
                let mut names = Vec::new();
                collect_infer_names(&extends_pattern, &mut names);
                let missing: Vec<&String> = names
                    .iter()
                    .filter(|name| member_substitution.get(name).is_none())
                    .collect();
                if !missing.is_empty() {
                    let shape = match &member {
                        Type::Reference(reference) => {
                            format!("Ref(args={})", reference.arguments.len())
                        }
                        Type::Object(_) => "Object".to_string(),
                        Type::Union(_) => "Union".to_string(),
                        other => format!("{other:?}").chars().take(30).collect(),
                    };
                    eprintln!(
                        "[had-error] infer-unbound {missing:?} member_shape={shape} member_name={} cp={}",
                        member.name(),
                        crate::program::in_check_phase()
                    );
                }
            }
            let resolved_branch = resolve_parsed_type(branch, ctx, resolving, &member_substitution);
            had_error |= resolved_branch.had_error;
            results.push(resolved_branch.ty);
        }

        return ResolvedType {
            ty: if results.is_empty() {
                Type::Never
            } else {
                union_type(results)
            },
            had_error,
        };
    }

    // A tuple pattern is decided by arity before the sentinel check below, because
    // a spread pattern has no fixed length of its own to compare against.
    match try_tuple_infer_match(
        &extends_pattern,
        &resolved_check.ty,
        substitution,
        ctx,
        resolving,
    ) {
        TuplePatternMatch::Matched(matched) => {
            return resolve_parsed_type(*conditional.true_type, ctx, resolving, &matched);
        }
        TuplePatternMatch::Rejected => {
            return resolve_parsed_type(*conditional.false_type, ctx, resolving, substitution);
        }
        TuplePatternMatch::Undecided => {}
    }

    // See the distributive arm: a degraded capture pattern decides nothing.
    if pattern_shape_degraded(&extends_pattern, &resolved_extends) {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    // Non-distributive: only evaluate when the check type is concrete enough for a
    // meaningful assignability test. An unresolved generic parameter resolves to
    // `Unknown`, which we treat as "cannot decide" and degrade. The *genuine*
    // `unknown` is a decision, not a failure — `unknown extends (…) => infer e` is
    // plainly false — and that distinction is the whole reason the two are
    // separate variants.
    // The same holds for a parameter *inside* the check type: `[Actual] extends
    // [(...args: any[]) => any]` with `Actual` still a placeholder is deferred
    // by tsc, not answered. Deciding it here answered `false` in a declaration
    // pre-pass, and the concrete shape that answer built (a brand object where
    // a method should be) was then reused for every real call.
    if (resolved_check.ty.is_unknown() && !matches!(resolved_check.ty, Type::GenuineUnknown))
        || contains_unresolved_parameter(&resolved_check.ty, SENTINEL_WALK_DEPTH)
        || (resolved_extends.ty.is_unknown()
            && !matches!(resolved_extends.ty, Type::GenuineUnknown))
    {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    // A type parameter bound to `any` on the extends side (`Inferrable extends
    // TInferrable`, called as `hook<typeof instance>()` where surge could not
    // model `instance`) would make every check type pass the constraint and
    // pick the true branch — the `TypeError<…>` arm such signatures reserve for
    // a *missing* argument. surge's `any` there is a modelling gap far more
    // often than a written `any`, so the branch is indeterminate.
    if matches!(resolved_extends.ty, Type::Any)
        && matches!(extends_pattern, ParsedType::Named(ref named)
            if named.type_arguments.is_empty() && substitution.get(&named.name).is_some())
    {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    // An `any` check type makes the branch indeterminate: tsc yields the union of
    // both branches, which with `any` in play collapses to an open `any` rather
    // than deterministically picking the true branch. Degrade to `any` so the
    // result stays open instead of resolving to a misleading concrete branch
    // (e.g. node's `_Request = typeof globalThis extends {...} ? {} : ...`).
    if matches!(resolved_check.ty, Type::Any) {
        return ResolvedType {
            ty: Type::Any,
            had_error: false,
        };
    }

    if is_assignable_to(&resolved_check.ty, &resolved_extends.ty) {
        let mut branch_substitution =
            substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        seed_infer_placeholders(&extends_pattern, &mut branch_substitution);
        bind_infer_captures(
            &extends_pattern,
            &resolved_check.ty,
            &mut branch_substitution,
            ctx,
            resolving,
            0,
            true,
        );
        resolve_parsed_type(*conditional.true_type, ctx, resolving, &branch_substitution)
    } else {
        resolve_parsed_type(*conditional.false_type, ctx, resolving, substitution)
    }
}

/// Structurally matches the parsed `extends` pattern against the resolved check
/// type, binding each `infer X` capture to the corresponding fragment so the
/// conditional's true branch can reference it. Handles the common
/// `Name<… infer X …>` and `Array<infer X>` shapes (recursing into nested
/// arguments); positions surge cannot line up are left unbound, so the branch
/// degrades like any other unresolved name rather than misbinding.
fn bind_infer_captures(
    extends: &ParsedType,
    check: &Type,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    depth: usize,
    reference_positional: bool,
) {
    match extends {
        ParsedType::Infer(name) => {
            substitution.insert(name.clone(), check.clone());
        }
        // `[infer head, ...infer tail]` / `[infer a, infer b]` against a tuple:
        // line up the fixed slots positionally and hand the spread slot the
        // middle as a tuple of its own. This is the list primitive every
        // type-level recursion is written with.
        ParsedType::Tuple(_) | ParsedType::VariadicTuple(_) | ParsedType::Readonly(_)
            if tuple_infer_enabled() =>
        {
            let peeled = crate::program::with_dts_expansion_reason(
                crate::program::DtsExpansionReason::ConditionalType,
                || check.peeled(),
            );
            if let Some(pairs) = tuple_pattern_pairs(tuple_pattern(extends), &peeled) {
                for (pattern_element, check_element) in pairs {
                    bind_infer_captures(
                        &pattern_element,
                        &check_element,
                        substitution,
                        ctx,
                        resolving,
                        depth,
                        reference_positional,
                    );
                }
            }
        }
        ParsedType::Array(element) => {
            let peeled = crate::program::with_dts_expansion_reason(
                crate::program::DtsExpansionReason::ConditionalType,
                || check.peeled(),
            );
            // A tuple *is* an array, so `T extends readonly (infer E)[]` captures
            // its element union — which is how a list walk flattens the tuple it
            // just built (`… extends readonly (infer T)[] ? T : never`). Matching
            // only `Type::Array` left the capture unbound and the whole walk
            // degraded one step later.
            let element_check = match &peeled {
                Type::Array(check_element) => Some(check_element.as_ref().clone()),
                Type::OpenTuple(open) => Some(open.element_union()),
                // The empty tuple has no element, and its element type is `never`
                // — which is what stops a list walk at its last step. An empty
                // union is not that: it is the degradation sentinel.
                Type::Tuple(members) if members.is_empty() => Some(Type::Never),
                Type::Tuple(members) => Some(union_type(members.clone())),
                _ => None,
            };
            if let Some(element_check) = element_check {
                bind_infer_captures(
                    element,
                    &element_check,
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
            }
        }
        // `T extends { File: infer F }` — line up each written member with the
        // check type's property of the same name. zod's `File` fallback
        // (`typeof globalThis extends { File: infer F … }`) captures this way.
        ParsedType::Object(pattern) => {
            let peeled = crate::program::with_dts_expansion_reason(
                crate::program::DtsExpansionReason::ConditionalType,
                || check.peeled(),
            );
            for property in &pattern.properties {
                if let Some(check_property) = peeled.get_property_access_type(&property.name) {
                    bind_infer_captures(
                        &property.ty,
                        &check_property,
                        substitution,
                        ctx,
                        resolving,
                        depth,
                        reference_positional,
                    );
                }
            }
            // An overloaded pattern (`{ (…): infer R1; (…): infer R2 }`) pairs
            // its signatures with the check type's overloads one for one,
            // aligned at the end the way tsc pairs signature lists. The parsed
            // fold that stands in for the group elsewhere widens every slot the
            // overloads disagree on, which is precisely where the captures are.
            if !pattern.call_signature_overloads.is_empty() {
                bind_overload_group_infer_captures(
                    &pattern.call_signature_overloads,
                    &peeled,
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
            } else if let Some(signature) = pattern.construct_signature.as_deref() {
                // A constructor *type* (`abstract new (...args: infer P) => any`,
                // `ConstructorParameters`' whole pattern) is an object carrying only
                // a construct signature, so its captures live there rather than in a
                // property — and it matches the check type's construct signatures,
                // not a call signature the same value may also carry (`Date()`).
                let check = match &peeled {
                    Type::Object(object) => object
                        .construct_signature()
                        .map(|construct| Type::Function(construct.clone()))
                        .unwrap_or_else(|| peeled.clone()),
                    _ => peeled.clone(),
                };
                bind_signature_infer_captures(
                    signature,
                    &check,
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
            } else if let Some(signature) = pattern.call_signature.as_deref() {
                bind_signature_infer_captures(
                    signature,
                    &peeled,
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
            }
        }
        // `(props: infer P) => infer R` matched against a concrete function type:
        // line up value parameters and the return position so captures inside a
        // function pattern bind. This recovers the props type for
        // `ComponentProps<typeof Component>`, whose `extends` clause is React's
        // `JSXElementConstructor<infer P>` (a union of function/constructor
        // signatures once the alias below is expanded).
        ParsedType::Function(pattern) => {
            let peeled = crate::program::with_dts_expansion_reason(
                crate::program::DtsExpansionReason::ConditionalType,
                || check.peeled(),
            );
            // Inference from `never` makes no candidate at all, and tsc lets such a
            // capture default to `unknown` — the *genuine* one, which then fails
            // the next `extends` test against a function type. That is what
            // terminates a union-peeling recursion on its last step; leaving the
            // degradation sentinel there instead makes the enclosing conditional
            // refuse to decide and the whole walk goes silent.
            if matches!(peeled, Type::Never) {
                let mut names = Vec::new();
                collect_infer_names(extends, &mut names);
                for name in names {
                    substitution.insert(name, Type::GenuineUnknown);
                }
                return;
            }
            if let Type::Union(union) = &peeled {
                bind_union_signature_infer_captures(
                    pattern,
                    union.types(),
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
                return;
            }
            bind_signature_infer_captures(
                pattern,
                &peeled,
                substitution,
                ctx,
                resolving,
                depth,
                reference_positional,
            );
        }
        // A union/intersection extends pattern (e.g. the body of
        // `JSXElementConstructor`) binds from whichever member structurally lines
        // up with the check type; members that do not align bind nothing.
        ParsedType::Union(members) | ParsedType::Intersection(members) => {
            for member in members.iter() {
                bind_infer_captures(
                    member,
                    check,
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
            }
        }
        ParsedType::Named(named) => {
            if named.type_arguments.is_empty() {
                return;
            }
            if named.name == "Array"
                && named.type_arguments.len() == 1
                && let Type::Array(element) = check
            {
                bind_infer_captures(
                    &named.type_arguments[0],
                    element,
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
                return;
            }
            // A same-shaped reference (`Box<infer T>` against `Ref(Box<number>)`)
            // binds positionally off the reference's own arguments. A reference
            // that carries NO arguments has nothing to line up, so it must fall
            // through to the structural expansion below instead of returning with
            // every capture still unbound — `ComponentProps<typeof Component>`
            // matches `JSXElementConstructor<infer Props>` against a
            // `ForwardRefExoticComponent` reference whose arguments live in its
            // resolved body, and returning here left `Props` as the seeded
            // placeholder, collapsing the whole conditional to `unknown`.
            // A reference to a *different* declaration (`StringChainable<p,
            // never>` against `Matcher<infer …>`) has nothing positional to
            // offer; it resolves toward the pattern's declaration below.
            let declaration_id = declaration_reference_id(&named.name, ctx);
            if reference_positional
                && let Type::Reference(reference) = check
                && !reference.arguments.is_empty()
                && declaration_id
                    .as_deref()
                    .is_none_or(|id| *reference.id == *id)
            {
                let check_arguments = complete_reference_arguments(named, reference, ctx, resolving);
                for (pattern_argument, check_argument) in
                    named.type_arguments.iter().zip(check_arguments.iter())
                {
                    bind_infer_captures(
                        pattern_argument,
                        check_argument,
                        substitution,
                        ctx,
                        resolving,
                        depth,
                        reference_positional,
                    );
                }
                return;
            }
            // An intersection source (`Matcher<unknown, string> & Omit<…>`)
            // merged to one object: the operand that references the pattern's
            // own declaration is what tsc infers from, positionally.
            if reference_positional
                && let Some(declaration_id) = declaration_id.as_deref()
                && let Some(operand) =
                    intersection_operand_for(check, declaration_id, OPERAND_SEARCH_DEPTH)
            {
                bind_infer_captures(
                    extends,
                    &operand,
                    substitution,
                    ctx,
                    resolving,
                    depth,
                    reference_positional,
                );
                return;
            }
            // The pattern is a generic alias whose argument carries an `infer`
            // capture (`JSXElementConstructor<infer P>`) but the check type is a
            // structural type (a function), not a same-named reference. Expand the
            // alias body one level — substituting its type parameters with the
            // pattern's arguments so the `infer` flows into the body — and match the
            // expanded shape structurally.
            if depth < INFER_ALIAS_EXPANSION_LIMIT
                && parsed_type_contains_infer(extends)
                && let Some(expanded) = expand_named_alias_pattern(named, ctx)
            {
                bind_infer_captures(
                    &expanded,
                    check,
                    substitution,
                    ctx,
                    resolving,
                    depth + 1,
                    reference_positional,
                );
            }
        }
        _ => {}
    }
}

/// How far the operand search follows alias references and nested merges:
/// `StringPattern` → `Chainable<…> & Omit<…>` → `GuardP<…> & Omit<…>` →
/// `Matcher<…>` is three levels.
const OPERAND_SEARCH_DEPTH: usize = 6;

/// The reference to `declaration_id` that `ty` is, or carries as an operand of
/// the intersection it merges — through alias references (`GuardP<a, b>` is a
/// `Matcher<a, b>`) and nested merges — with its arguments.
fn intersection_operand_for(ty: &Type, declaration_id: &str, depth: usize) -> Option<Type> {
    if depth == 0 {
        return None;
    }
    match ty {
        Type::Reference(reference) => {
            if *reference.id == *declaration_id && !reference.arguments.is_empty() {
                return Some(ty.clone());
            }
            intersection_operand_for(&reference.resolve(), declaration_id, depth - 1)
        }
        Type::Object(object) => object
            .intersection_operands
            .as_deref()?
            .iter()
            .find_map(|operand| intersection_operand_for(operand, declaration_id, depth - 1)),
        _ => None,
    }
}

/// The nominal id (`file\0Name`) a lazy reference to `name`'s declaration
/// carries, as seen from the current scope.
fn declaration_reference_id(name: &str, ctx: &CheckerContext) -> Option<String> {
    let handle = ctx.lookup_type_declaration_handle(name)?;
    let (file_name, declaration_name) = match handle.get() {
        crate::symbols::TypeDeclarationInfo::Alias(alias) => (alias.file_name.clone(), alias.name.clone()),
        crate::symbols::TypeDeclarationInfo::Interface(interface) => {
            (interface.file_name.clone(), interface.name.clone())
        }
    };
    Some(format!("{file_name}\u{0}{declaration_name}"))
}

/// The reference's arguments followed by the declaration's resolved defaults
/// for the parameters it left out, so a pattern with more slots than the
/// reference wrote (`M<infer i, infer n, infer mt>` against `M<number,
/// string>`) lines up the way tsc's `M<number, string, "default">` does. The
/// written list is kept whenever a default cannot be bound; diagnostics the
/// probe emits are rolled back.
fn complete_reference_arguments(
    named: &ParsedNamedType,
    reference: &surge_ts_types::TypeReference,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
) -> Vec<Type> {
    let written = reference.arguments.to_vec();
    if named.type_arguments.len() <= written.len() {
        return written;
    }
    let Some(handle) = ctx.lookup_type_declaration_handle(&named.name) else {
        return written;
    };
    let (type_parameters, scope, file_name, name, name_span, declared_name) = match handle.get() {
        crate::symbols::TypeDeclarationInfo::Alias(alias) => (
            alias.body.type_parameters.clone(),
            alias.resolution_scope.clone(),
            alias.file_name.clone(),
            alias.name.clone(),
            alias.name_span,
            alias.declared_name.clone(),
        ),
        crate::symbols::TypeDeclarationInfo::Interface(interface) => (
            interface.body.type_parameters.clone(),
            interface.resolution_scope.clone(),
            interface.file_name.clone(),
            interface.name.clone(),
            interface.name_span,
            interface.declared_name.clone(),
        ),
    };
    if type_parameters.len() <= written.len() {
        return written;
    }
    let declaration_scope = scope.or_else(|| {
        ctx.module_scope_for_file(&file_name)
            .filter(|scope| !scope.is_empty())
    });
    let prefix =
        crate::infer::types::utility::namespace_member_prefix(declared_name.as_deref(), &name);
    if let Some(prefix) = prefix.clone() {
        ctx.namespace_member_resolution_depth += 1;
        ctx.namespace_member_prefix_stack.push(prefix);
    }
    let diagnostics_before = ctx.diagnostics().len();
    let bound = bind_type_arguments(
        &type_parameters,
        vec![ParsedType::Never; written.len()],
        &name,
        name_span,
        ctx,
        resolving,
        &TypeParameterSubstitution::new(),
        Some(&written),
        Some((&declaration_scope, &file_name)),
    );
    ctx.truncate_diagnostics_releasing_utility_keys(diagnostics_before);
    if prefix.is_some() {
        ctx.namespace_member_resolution_depth -= 1;
        ctx.namespace_member_prefix_stack.pop();
    }
    let Some(bound) = bound.filter(|bound| !bound.had_error) else {
        return written;
    };
    type_parameters
        .iter()
        .map(|parameter| bound.substitution.get(&parameter.name).cloned())
        .collect::<Option<Vec<_>>>()
        .unwrap_or(written)
}

/// `SURGE_VARIADIC_TUPLES=0` also turns off tuple `extends`-pattern inference, so
/// one switch restores the whole pre-change behaviour: a fixed tuple pattern's
/// captures stay unbound and the branch is chosen by plain assignability, as it
/// was before tuple slots could be lined up at all.
fn tuple_infer_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_VARIADIC_TUPLES").as_deref() != Ok("0"))
}

enum DeferredIdentity {
    NotThisShape,
    Identical(bool),
    Undecidable,
}

/// Whether a conditional is comparing two *deferred* conditionals for identity,
/// and if so whether they are identical. A side that carries a modelling gap
/// makes the test undecidable: answering either way would be a guess.
fn deferred_conditional_identity(
    check: &ParsedType,
    extends: &ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> DeferredIdentity {
    let (ParsedType::Function(left), ParsedType::Function(right)) = (check, extends) else {
        return DeferredIdentity::NotThisShape;
    };
    let (Some(left_body), Some(right_body)) = (
        deferred_parameter_conditional(left),
        deferred_parameter_conditional(right),
    ) else {
        return DeferredIdentity::NotThisShape;
    };
    if left.parameters.len() != right.parameters.len() {
        return DeferredIdentity::NotThisShape;
    }

    let mut identical = true;
    for (left_part, right_part) in [
        (&left_body.extends_type, &right_body.extends_type),
        (&left_body.true_type, &right_body.true_type),
        (&left_body.false_type, &right_body.false_type),
    ] {
        let left_resolved = resolve_parsed_type((**left_part).clone(), ctx, resolving, substitution);
        let right_resolved =
            resolve_parsed_type((**right_part).clone(), ctx, resolving, substitution);
        if left_resolved.had_error || right_resolved.had_error {
            return DeferredIdentity::Undecidable;
        }
        match identity_compare(&left_resolved.ty, &right_resolved.ty, SENTINEL_WALK_DEPTH) {
            Some(equal) => identical &= equal,
            None => return DeferredIdentity::Undecidable,
        }
    }

    DeferredIdentity::Identical(identical)
}

/// Whether `ty` is (or lazily resolves to) the readonly array/tuple wrapper.
fn is_readonly_shape(ty: &Type) -> bool {
    let mut current = ty.clone();
    for _ in 0..8 {
        match current {
            Type::Reference(reference) if reference.is_readonly_array() => return true,
            Type::Reference(reference) => current = reference.resolve(),
            _ => return false,
        }
    }
    false
}

/// The tuple pattern under a `readonly` modifier: `readonly [infer a, ...infer b]`
/// lines up its slots exactly like the mutable spelling (a mutable tuple is
/// assignable to the readonly pattern).
fn tuple_pattern(extends: &ParsedType) -> &ParsedType {
    match extends {
        ParsedType::Readonly(inner) => inner,
        other => other,
    }
}

/// tsc's identity relation is structural: `Id<{ a: 1 }>` and `{ a: 1 }` are
/// the same type. `Type::Reference` equality is nominal, so a lazily
/// referenced alias instantiation has to be peeled before it can match the
/// shape it stands for, on either side and at every nesting level a type-level
/// test looks at.
///
/// Three-valued: `None` means the comparison reached a shape surge could not
/// model — the degradation sentinel, an unbound parameter, or `any`, which is
/// far more often surge's answer for an inference it could not make than a
/// written one — *at a position where the two sides otherwise agree*, so
/// neither answer would be honest. A difference decided before that point
/// (a union against an object, a missing key, a different literal) is a real
/// difference whatever sits deeper: ts-pattern's `Pattern<unknown>` carries
/// written `any`s and still is not `StringPattern`. Past the depth bound the
/// answer is "cannot tell".
fn identity_compare(left: &Type, right: &Type, depth: usize) -> Option<bool> {
    if left == right {
        return Some(true);
    }
    if depth == 0 {
        return None;
    }
    let readonly_differs = is_readonly_shape(left) != is_readonly_shape(right);
    let (left, right) = (left.peeled(), right.peeled());
    if left == right {
        return Some(true);
    }
    // A side surge could not model decides nothing, not even against the
    // readonly modifier of the other side.
    let undecidable = |ty: &Type| matches!(ty, Type::Unknown | Type::TypeParameter(_) | Type::Any);
    if undecidable(&left) || undecidable(&right) {
        return None;
    }
    if readonly_differs {
        return Some(false);
    }
    match (&left, &right) {
        (Type::Object(left), Type::Object(right)) => {
            if left.properties.len() != right.properties.len()
                || left.string_index_type.is_some() != right.string_index_type.is_some()
                || left.call_signature != right.call_signature
                || left.construct_signature != right.construct_signature
            {
                return Some(false);
            }
            let mut undecided = false;
            for (name, property) in left.properties.iter() {
                let Some(other) = right.properties.get(name) else {
                    return Some(false);
                };
                if property.optional != other.optional || property.method != other.method {
                    return Some(false);
                }
                match identity_compare(&property.ty, &other.ty, depth - 1) {
                    Some(true) => {}
                    Some(false) => return Some(false),
                    None => undecided = true,
                }
            }
            if let (Some(l), Some(r)) = (&left.string_index_type, &right.string_index_type) {
                match identity_compare(l, r, depth - 1) {
                    Some(true) => {}
                    Some(false) => return Some(false),
                    None => undecided = true,
                }
            }
            (!undecided).then_some(true)
        }
        (Type::Union(left), Type::Union(right)) => {
            if left.types().len() != right.types().len() {
                return Some(false);
            }
            let mut undecided = false;
            for member in left.types().iter() {
                let mut found = false;
                let mut member_undecided = false;
                for other in right.types().iter() {
                    match identity_compare(member, other, depth - 1) {
                        Some(true) => {
                            found = true;
                            break;
                        }
                        Some(false) => {}
                        None => member_undecided = true,
                    }
                }
                if !found {
                    if member_undecided {
                        undecided = true;
                    } else {
                        return Some(false);
                    }
                }
            }
            (!undecided).then_some(true)
        }
        (Type::Array(left), Type::Array(right)) => identity_compare(left, right, depth - 1),
        (Type::Tuple(left), Type::Tuple(right)) => {
            if left.len() != right.len() {
                return Some(false);
            }
            let mut undecided = false;
            for (l, r) in left.iter().zip(right.iter()) {
                match identity_compare(l, r, depth - 1) {
                    Some(true) => {}
                    Some(false) => return Some(false),
                    None => undecided = true,
                }
            }
            (!undecided).then_some(true)
        }
        (Type::Function(left), Type::Function(right)) => {
            if left.parameters().len() != right.parameters().len() {
                return Some(false);
            }
            let mut undecided = false;
            for (l, r) in left
                .parameters()
                .iter()
                .zip(right.parameters().iter())
                .chain(std::iter::once((left.return_type(), right.return_type())))
            {
                match identity_compare(l, r, depth - 1) {
                    Some(true) => {}
                    Some(false) => return Some(false),
                    None => undecided = true,
                }
            }
            (!undecided).then_some(true)
        }
        _ => Some(false),
    }
}

/// Whether an `extends` pattern with `infer` captures resolved to a shape
/// surge could not model: the resolution itself degraded, or a lazily
/// resolved member of it did (`Matcher<infer …>` whose `[matcher]` member
/// degrades on peel). An assignability test against such a shape is
/// permissive for the wrong reason — a member that matches nothing would
/// take the true branch and bind nothing — so the conditional degrades.
fn pattern_shape_degraded(extends_pattern: &ParsedType, resolved_extends: &ResolvedType) -> bool {
    parsed_type_contains_infer(extends_pattern)
        && (resolved_extends.had_error
            || contains_unresolved_parameter(&resolved_extends.ty, SENTINEL_WALK_DEPTH))
}

/// How deep `identity_compare` looks before giving up and calling the type
/// undecidable. Bounded so a lazy self-referential shape cannot be
/// walked forever; four levels covers the handler-parameter shapes a type-level
/// test compares (`{ type: 'some'; value: { list: … } }`).
const SENTINEL_WALK_DEPTH: usize = 4;

/// Whether an unresolved type parameter or the degradation sentinel sits
/// anywhere inside `ty` (bounded like `identity_compare`; `any` is
/// a decision of its own and is not counted here).
fn contains_unresolved_parameter(ty: &Type, depth: usize) -> bool {
    match ty {
        Type::Unknown | Type::TypeParameter(_) => true,
        Type::Any | Type::GenuineUnknown => false,
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| contains_unresolved_parameter(member, depth)),
        Type::Array(element) => contains_unresolved_parameter(element, depth),
        Type::Tuple(elements) => elements
            .iter()
            .any(|element| contains_unresolved_parameter(element, depth)),
        Type::OpenTuple(open) => {
            open.leading
                .iter()
                .chain(open.trailing.iter())
                .any(|element| contains_unresolved_parameter(element, depth))
                || contains_unresolved_parameter(&open.rest, depth)
        }
        Type::Reference(_) => {
            if depth == 0 {
                return true;
            }
            let peeled = crate::program::with_dts_expansion_reason(
                crate::program::DtsExpansionReason::ConditionalType,
                || ty.peeled(),
            );
            contains_unresolved_parameter(&peeled, depth - 1)
        }
        _ => false,
    }
}


/// The signature's return conditional, when the signature is generic and that
/// conditional tests its own type parameter — the shape whose evaluation tsc
/// defers.
fn deferred_parameter_conditional(
    function: &surge_ts_syntax::ParsedFunctionType,
) -> Option<&surge_ts_syntax::ParsedConditionalType> {
    if function.type_parameters.is_empty() {
        return None;
    }
    let ParsedType::Conditional(body) = &*function.return_type else {
        return None;
    };
    let ParsedType::Named(tested) = body.check_type.as_ref() else {
        return None;
    };
    function
        .type_parameters
        .iter()
        .any(|parameter| parameter.name == tested.name && tested.type_arguments.is_empty())
        .then_some(body.as_ref())
}

fn tuple_element_type(element: &surge_ts_syntax::ParsedTupleElement) -> &ParsedType {
    let (surge_ts_syntax::ParsedTupleElement::Fixed(ty)
    | surge_ts_syntax::ParsedTupleElement::Rest(ty)) = element;
    ty
}

/// How a tuple `extends` pattern lines up against the check type.
enum TuplePatternMatch {
    /// The pattern is not a tuple one, or the check type's shape cannot decide
    /// it — leave the existing branch test alone.
    Undecided,
    Matched(TypeParameterSubstitution),
    /// The check type is a tuple the pattern cannot match: the false branch.
    Rejected,
}

/// Branch test for a conditional whose `extends` pattern is a tuple carrying an
/// `infer` capture. A spread pattern (`[infer head, ...infer tail]`) has no fixed
/// length, so it resolves to the `unknown` sentinel and the assignability test
/// cannot decide the branch at all — arity decides it here instead, which is what
/// lets a recursive list walk terminate on the empty tuple rather than matching
/// forever. Patterns with no capture keep the plain assignability test.
fn try_tuple_infer_match(
    extends: &ParsedType,
    check: &Type,
    base: &TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
) -> TuplePatternMatch {
    let extends = tuple_pattern(extends);
    if !tuple_infer_enabled()
        || !matches!(extends, ParsedType::Tuple(_) | ParsedType::VariadicTuple(_))
        || !parsed_type_contains_infer(extends)
    {
        return TuplePatternMatch::Undecided;
    }

    let peeled = crate::program::with_dts_expansion_reason(
        crate::program::DtsExpansionReason::ConditionalType,
        || check.peeled(),
    );
    if !matches!(peeled, Type::Tuple(_) | Type::Array(_) | Type::OpenTuple(_)) {
        return TuplePatternMatch::Undecided;
    }

    let Some(pairs) = tuple_pattern_pairs(extends, &peeled) else {
        return TuplePatternMatch::Rejected;
    };

    // A slot written as a concrete type still has to hold: `T extends [string,
    // ...infer rest]` must not match `[number]` just because the arity lines up.
    // A slot whose capture sits inside a generic reference the check member does
    // not line up with (`[TResult] extends [MutationState<infer TData, …>]`) leaves
    // that name unbound. Seed every capture first so it reads as unresolved in the
    // true branch instead of reporting a false TS2304 for the capture name — the
    // same reason `seed_infer_placeholders` exists on the assignability path.
    let mut candidate = base.clone_with_reason(TypeCopyReason::SubstitutionChanged);
    seed_infer_placeholders(extends, &mut candidate);
    for (pattern_element, check_element) in &pairs {
        if parsed_type_contains_infer(pattern_element) {
            continue;
        }
        let resolved_element = resolve_parsed_type(pattern_element.clone(), ctx, resolving, base);
        // A slot written as the genuine `unknown` (`[...infer R, unknown]`, the
        // shape `DropLast` is written with) accepts anything; only the
        // degradation sentinel means the slot could not be modelled.
        if resolved_element.had_error
            || matches!(resolved_element.ty, Type::Unknown | Type::TypeParameter(_))
        {
            return TuplePatternMatch::Undecided;
        }
        if !is_assignable_to(check_element, &resolved_element.ty) {
            return TuplePatternMatch::Rejected;
        }
    }

    for (pattern_element, check_element) in pairs {
        bind_infer_captures(
            &pattern_element,
            &check_element,
            &mut candidate,
            ctx,
            resolving,
            0,
            true,
        );
    }

    TuplePatternMatch::Matched(candidate)
}

/// Pairs each slot of a tuple `extends` pattern with the part of the check tuple
/// it captures. `None` when the pattern cannot match: a length mismatch against a
/// fixed pattern, too few elements for a spread pattern's fixed slots, or a
/// pattern shape with more than one spread (which is not inferable).
fn tuple_pattern_pairs(extends: &ParsedType, check: &Type) -> Option<Vec<(ParsedType, Type)>> {
    let elements = match extends {
        ParsedType::Tuple(elements) => elements
            .iter()
            .cloned()
            .map(surge_ts_syntax::ParsedTupleElement::Fixed)
            .collect::<Vec<_>>(),
        ParsedType::VariadicTuple(elements) => elements.as_ref().clone(),
        _ => return None,
    };

    let rest_positions = elements
        .iter()
        .enumerate()
        .filter(|(_, element)| matches!(element, surge_ts_syntax::ParsedTupleElement::Rest(_)))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if rest_positions.len() > 1 {
        return None;
    }

    let members = match check {
        Type::Tuple(members) => members.clone(),
        // `T[] extends [...infer rest]` captures the array itself; any pattern
        // with a fixed slot needs a length an array does not have.
        Type::Array(_) if elements.len() == 1 && rest_positions.len() == 1 => {
            let surge_ts_syntax::ParsedTupleElement::Rest(written) = &elements[0] else {
                return None;
            };
            return Some(vec![(written.clone(), check.clone())]);
        }
        // An open tuple lines its fixed slots up with the pattern's; the spread
        // slot receives whatever is left, which is again an open tuple (or the
        // bare array once no fixed slot remains).
        Type::OpenTuple(open) => {
            let Some(&rest_index) = rest_positions.first() else {
                return None;
            };
            let leading = &elements[..rest_index];
            let trailing = &elements[rest_index + 1..];
            if open.leading.len() < leading.len() || open.trailing.len() < trailing.len() {
                return None;
            }
            let mut pairs = Vec::with_capacity(elements.len());
            for (element, member) in leading.iter().zip(open.leading.iter()) {
                let surge_ts_syntax::ParsedTupleElement::Fixed(written) = element else {
                    return None;
                };
                pairs.push((written.clone(), member.clone()));
            }
            let surge_ts_syntax::ParsedTupleElement::Rest(rest_written) = &elements[rest_index]
            else {
                return None;
            };
            let kept_trailing = open.trailing.len() - trailing.len();
            let remainder = surge_ts_types::OpenTupleType {
                leading: open.leading[leading.len()..].to_vec(),
                rest: open.rest.clone(),
                trailing: open.trailing[..kept_trailing].to_vec(),
            };
            let remainder = if remainder.fixed_len() == 0 {
                Type::Array(remainder.rest)
            } else {
                Type::OpenTuple(remainder)
            };
            pairs.push((rest_written.clone(), remainder));
            for (element, member) in trailing.iter().zip(open.trailing[kept_trailing..].iter()) {
                let surge_ts_syntax::ParsedTupleElement::Fixed(written) = element else {
                    return None;
                };
                pairs.push((written.clone(), member.clone()));
            }
            return Some(pairs);
        }
        _ => return None,
    };

    let Some(&rest_index) = rest_positions.first() else {
        if members.len() != elements.len() {
            return None;
        }
        return Some(
            elements
                .into_iter()
                .zip(members)
                .map(|(element, member)| match element {
                    surge_ts_syntax::ParsedTupleElement::Fixed(written) => (written, member),
                    surge_ts_syntax::ParsedTupleElement::Rest(written) => (written, member),
                })
                .collect(),
        );
    };

    let leading = &elements[..rest_index];
    let trailing = &elements[rest_index + 1..];
    if members.len() < leading.len() + trailing.len() {
        return None;
    }

    let mut pairs = Vec::with_capacity(elements.len());
    for (element, member) in leading.iter().zip(members.iter()) {
        let surge_ts_syntax::ParsedTupleElement::Fixed(written) = element else {
            return None;
        };
        pairs.push((written.clone(), member.clone()));
    }

    let middle_end = members.len() - trailing.len();
    let surge_ts_syntax::ParsedTupleElement::Rest(rest_written) = &elements[rest_index] else {
        return None;
    };
    pairs.push((
        rest_written.clone(),
        Type::Tuple(members[leading.len()..middle_end].to_vec()),
    ));

    for (offset, element) in trailing.iter().enumerate() {
        let surge_ts_syntax::ParsedTupleElement::Fixed(written) = element else {
            return None;
        };
        pairs.push((written.clone(), members[middle_end + offset].clone()));
    }

    Some(pairs)
}

/// Maximum alias-expansion depth while structurally matching an `extends` pattern
/// against the check type. Bounds pathological self-referential aliases; real
/// patterns (`JSXElementConstructor<infer P>`) need a single level.
const INFER_ALIAS_EXPANSION_LIMIT: usize = 8;

/// Fallback branch test for a conditional whose `extends` pattern captures inside
/// a function position (`T extends JSXElementConstructor<infer P> ? P : …`) but
/// whose resolved form degraded to `unknown` — so the assignability-based test
/// could not select the true branch. When the check type is a function (a
/// component value for `ComponentProps<typeof Component>`), structurally match the
/// pattern and, if any `infer` capture binds, return the extended substitution so
/// the caller takes the true branch. Reference-by-position matching is disabled
/// here, so an unrelated same-arity generic (`Promise<string>` against
/// `Array<infer U>`) can never spuriously bind.
fn try_function_infer_match(
    extends: &ParsedType,
    check: &Type,
    base: &TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
) -> Option<TypeParameterSubstitution> {
    if callable_signature(&crate::program::with_dts_expansion_reason(
        crate::program::DtsExpansionReason::ConditionalType,
        || check.peeled(),
    ))
    .is_none()
    {
        return None;
    }

    let mut infer_names = Vec::new();
    collect_infer_names(extends, &mut infer_names);
    if infer_names.is_empty() {
        return None;
    }

    let mut candidate = base.clone_with_reason(TypeCopyReason::SubstitutionChanged);
    bind_infer_captures(extends, check, &mut candidate, ctx, resolving, 0, false);

    if infer_names.iter().any(|name| candidate.get(name).is_some()) {
        Some(candidate)
    } else {
        None
    }
}

/// Inference from a *union* of signatures into one written signature. tsc infers
/// from every constituent and then combines the candidates by the position they
/// were found in: a capture in a parameter position is contravariant, so its
/// candidates intersect, and one in the return position is covariant, so they
/// union. That contravariant intersection is the whole mechanism behind
/// `UnionToIntersection` — `(u extends any ? (k: u) => void : never) extends
/// (k: infer i) => void` — which is how most of the ecosystem spells "turn this
/// union into an intersection". Without it the union check type matched no
/// callable at all and every capture stayed unbound.
#[allow(clippy::too_many_arguments)]
fn bind_union_signature_infer_captures(
    pattern: &surge_ts_syntax::ParsedFunctionType,
    members: &[Type],
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    depth: usize,
    reference_positional: bool,
) {
    let mut variance = Vec::new();
    for parameter in pattern.parameters.iter().filter(|it| !it.is_this) {
        collect_infer_variance(&parameter.ty, true, &mut variance);
    }
    collect_infer_variance(&pattern.return_type, false, &mut variance);
    if variance.is_empty() {
        return;
    }

    let mut candidates: Vec<Vec<Type>> = vec![Vec::new(); variance.len()];
    for member in members {
        let peeled = crate::program::with_dts_expansion_reason(
            crate::program::DtsExpansionReason::ConditionalType,
            || member.peeled(),
        );
        if callable_signature(&peeled).is_none() {
            continue;
        }
        // A fresh map per constituent: the caller's substitution already carries a
        // seeded placeholder for every capture, so binding into it would make
        // "this member contributed nothing" indistinguishable from "it bound the
        // seed".
        let mut member_captures = TypeParameterSubstitution::new();
        bind_signature_infer_captures(
            pattern,
            &peeled,
            &mut member_captures,
            ctx,
            resolving,
            depth,
            reference_positional,
        );
        for (index, (name, _)) in variance.iter().enumerate() {
            if let Some(bound) = member_captures.get(name) {
                candidates[index].push(bound.clone());
            }
        }
    }

    for (index, (name, contravariant)) in variance.iter().enumerate() {
        let bound = std::mem::take(&mut candidates[index]);
        if bound.is_empty() {
            continue;
        }
        let combined = if *contravariant {
            super::intersection::merge_intersection_members(bound)
        } else {
            union_type(bound)
        };
        substitution.insert(name.clone(), combined);
    }
}

/// Every `infer` capture in a written pattern with the variance of the position
/// it sits in: `true` for contravariant (under an odd number of parameter
/// positions), `false` for covariant.
fn collect_infer_variance(ty: &ParsedType, contravariant: bool, out: &mut Vec<(String, bool)>) {
    match ty {
        ParsedType::Infer(name) => out.push((name.clone(), contravariant)),
        ParsedType::Function(function) => {
            for parameter in function.parameters.iter().filter(|it| !it.is_this) {
                collect_infer_variance(&parameter.ty, !contravariant, out);
            }
            collect_infer_variance(&function.return_type, contravariant, out);
        }
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => {
            collect_infer_variance(inner, contravariant, out)
        }
        ParsedType::Union(members)
        | ParsedType::Intersection(members)
        | ParsedType::Tuple(members) => {
            for member in members.iter() {
                collect_infer_variance(member, contravariant, out);
            }
        }
        ParsedType::VariadicTuple(elements) => {
            for element in elements.iter() {
                collect_infer_variance(tuple_element_type(element), contravariant, out);
            }
        }
        ParsedType::Readonly(inner) => collect_infer_variance(inner, contravariant, out),
        ParsedType::Named(named) => {
            for argument in &named.type_arguments {
                collect_infer_variance(argument, contravariant, out);
            }
        }
        ParsedType::Object(object) => {
            for property in &object.properties {
                collect_infer_variance(&property.ty, contravariant, out);
            }
            for signature in object
                .construct_signature
                .as_deref()
                .into_iter()
                .chain(object.call_signature.as_deref())
            {
                for parameter in signature.parameters.iter().filter(|it| !it.is_this) {
                    collect_infer_variance(&parameter.ty, !contravariant, out);
                }
                collect_infer_variance(&signature.return_type, contravariant, out);
            }
        }
        ParsedType::Predicate(predicate) => {
            if let Some(ty) = &predicate.ty {
                collect_infer_variance(ty, contravariant, out);
            }
        }
        _ => {}
    }
}

/// Binds the captures of an overloaded call-signature pattern. Target
/// signature `i` is paired with the check type's overload at the same distance
/// from the end, which is how tsc lines up two signature lists; with fewer
/// source overloads than the pattern writes, the extra patterns all read the
/// last one rather than binding nothing.
fn bind_overload_group_infer_captures(
    patterns: &[surge_ts_syntax::ParsedFunctionType],
    check: &Type,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    depth: usize,
    reference_positional: bool,
) {
    let Some(check_function) = callable_signature(check) else {
        return;
    };
    let overloads = check_function.overloads().unwrap_or_default();
    for (index, pattern) in patterns.iter().enumerate() {
        let check_signature = if overloads.len() >= patterns.len() {
            overloads.get(overloads.len() - patterns.len() + index)
        } else {
            overloads.get(index).or_else(|| overloads.last())
        };
        let check_type = match check_signature {
            Some(signature) => Type::Function(signature.clone()),
            None => check.clone(),
        };
        bind_signature_infer_captures(
            pattern,
            &check_type,
            substitution,
            ctx,
            resolving,
            depth,
            reference_positional,
        );
    }
}

/// The callable signature of a check type, treating a function and a callable
/// object (one carrying a call or construct signature, e.g. React's
/// `ForwardRefExoticComponent<P>` or a class value) uniformly. This lets
/// `JSXElementConstructor<infer P>` recover the props type from a `forwardRef`/
/// `memo` component, not only from a plain function component.
/// Lines up a written signature pattern with the check type's callable surface:
/// value parameters positionally, the rest parameter against the tuple of every
/// remaining one (what makes `Parameters`/`ConstructorParameters` yield a
/// parameter list), and the return position.
#[allow(clippy::too_many_arguments)]
fn bind_signature_infer_captures(
    pattern: &surge_ts_syntax::ParsedFunctionType,
    check: &Type,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    depth: usize,
    reference_positional: bool,
) {
    let Some(check_function) = callable_signature(check) else {
        return;
    };
    // An overload group infers from its *last* signature, matching tsc: with one
    // target signature it pairs the tail of the source list. The fold itself is a
    // permissive shape whose return degrades whenever two overloads disagree, so
    // inferring from it would lose exactly what the caller asked for.
    let check_function = check_function
        .overloads()
        .and_then(<[surge_ts_types::FunctionType]>::last)
        .unwrap_or(check_function);
    let check_parameters = check_function.parameters();
    let pattern_parameters = pattern
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_this);
    for (index, pattern_parameter) in pattern_parameters.enumerate() {
        if pattern_parameter.rest {
            bind_infer_captures(
                &pattern_parameter.ty,
                &remaining_parameters_type(check_function, index),
                substitution,
                ctx,
                resolving,
                depth,
                reference_positional,
            );
            break;
        }
        if let Some(check_parameter) = check_parameters.get(index) {
            bind_infer_captures(
                &pattern_parameter.ty,
                check_parameter,
                substitution,
                ctx,
                resolving,
                depth,
                reference_positional,
            );
        }
    }
    bind_infer_captures(
        &pattern.return_type,
        check_function.return_type(),
        substitution,
        ctx,
        resolving,
        depth,
        reference_positional,
    );
}

/// The parameter list a rest pattern captures from `remaining`. A check
/// signature's own rest parameter is stored as its array type, so it is the
/// capture itself rather than a one-element tuple around it (`Parameters<(...a:
/// any[]) => R>` is `any[]`). Fixed parameters ahead of a rest have no variadic
/// tuple to land in here; they widen into the element union instead.
fn remaining_parameters_type(check_function: &surge_ts_types::FunctionType, start: usize) -> Type {
    let parameters = check_function.parameters();
    let start = start.min(parameters.len());
    let remaining = &parameters[start..];
    if !check_function.is_variadic() {
        return crate::infer::types::utility::optional_parameter_tuple(
            remaining,
            check_function
                .required_parameter_count()
                .saturating_sub(start),
            false,
        );
    }
    match remaining {
        [] => Type::Tuple(Vec::new()),
        [rest] => rest.clone(),
        [fixed @ .., rest] => {
            let element = match rest.peeled() {
                Type::Array(element) => element.as_ref().clone(),
                other => other,
            };
            let mut members = fixed.to_vec();
            members.push(element);
            Type::Array(Box::new(surge_ts_types::union_type(members)))
        }
    }
}

fn callable_signature(ty: &Type) -> Option<&surge_ts_types::FunctionType> {
    match ty {
        Type::Function(function) => Some(function),
        Type::Object(object) => object
            .call_signature()
            .or_else(|| object.construct_signature()),
        _ => None,
    }
}

/// Seeds every `infer X` capture in the pattern as a degradation placeholder
/// before matching. `bind_infer_captures` overwrites the positions it can line
/// up; the ones it cannot (a union check type against a function pattern, the
/// shape Prisma's `IntersectOf` uses) then read as unresolved rather than
/// reporting a false TS2304 for the capture name in the true branch.
fn seed_infer_placeholders(pattern: &ParsedType, substitution: &mut TypeParameterSubstitution) {
    let mut names = Vec::new();
    collect_infer_names(pattern, &mut names);
    for name in names {
        if substitution.get(&name).is_none() {
            let placeholder = Type::type_parameter(&name);
            substitution.insert_placeholder(name, placeholder);
        }
    }
}

/// Collects every `infer X` capture name reachable inside a parsed `extends`
/// pattern, recursing through the same structural positions as
/// [`bind_infer_captures`].
fn collect_infer_names(ty: &ParsedType, names: &mut Vec<String>) {
    match ty {
        ParsedType::Infer(name) => names.push(name.clone()),
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => collect_infer_names(inner, names),
        ParsedType::Union(members)
        | ParsedType::Intersection(members)
        | ParsedType::Tuple(members) => {
            for member in members.iter() {
                collect_infer_names(member, names);
            }
        }
        ParsedType::VariadicTuple(elements) => {
            for element in elements.iter() {
                collect_infer_names(tuple_element_type(element), names);
            }
        }
        ParsedType::Readonly(inner) => collect_infer_names(inner, names),
        ParsedType::Function(function) => {
            for parameter in &function.parameters {
                collect_infer_names(&parameter.ty, names);
            }
            collect_infer_names(&function.return_type, names);
        }
        // `(value: any) => value is infer narrowed`. The capture sits in the
        // predicate, not in a plain return type, so without this arm the name is
        // never seeded and the true branch resolves it as an unknown type name.
        ParsedType::Predicate(predicate) => {
            if let Some(ty) = &predicate.ty {
                collect_infer_names(ty, names);
            }
        }
        ParsedType::Named(named) => {
            for argument in &named.type_arguments {
                collect_infer_names(argument, names);
            }
        }
        ParsedType::Object(object) => {
            for property in &object.properties {
                collect_infer_names(&property.ty, names);
            }
            for signature in object
                .construct_signature
                .as_deref()
                .into_iter()
                .chain(object.call_signature.as_deref())
            {
                for parameter in &signature.parameters {
                    collect_infer_names(&parameter.ty, names);
                }
                collect_infer_names(&signature.return_type, names);
            }
        }
        _ => {}
    }
}

/// Whether a parsed type mentions an `infer X` capture anywhere within it. Used
/// to gate the (more expensive) alias-expansion path in [`bind_infer_captures`]
/// to patterns that actually capture.
fn parsed_type_contains_infer(ty: &ParsedType) -> bool {
    match ty {
        ParsedType::Infer(_) => true,
        ParsedType::Object(object) => {
            object
                .properties
                .iter()
                .any(|property| parsed_type_contains_infer(&property.ty))
                || object
                    .construct_signature
                    .as_deref()
                    .into_iter()
                    .chain(object.call_signature.as_deref())
                    .any(|signature| {
                        signature
                            .parameters
                            .iter()
                            .any(|parameter| parsed_type_contains_infer(&parameter.ty))
                            || parsed_type_contains_infer(&signature.return_type)
                    })
        }
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) | ParsedType::Readonly(inner) => {
            parsed_type_contains_infer(inner)
        }
        ParsedType::Union(members)
        | ParsedType::Intersection(members)
        | ParsedType::Tuple(members) => members.iter().any(parsed_type_contains_infer),
        ParsedType::VariadicTuple(elements) => elements
            .iter()
            .map(tuple_element_type)
            .any(parsed_type_contains_infer),
        ParsedType::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| parsed_type_contains_infer(&parameter.ty))
                || parsed_type_contains_infer(&function.return_type)
        }
        ParsedType::Named(named) => named.type_arguments.iter().any(parsed_type_contains_infer),
        ParsedType::Predicate(predicate) => predicate
            .ty
            .as_ref()
            .is_some_and(parsed_type_contains_infer),
        _ => false,
    }
}

/// Expands a generic alias reference (`Name<A, B>`) to its declared body with the
/// alias's type parameters textually substituted by the reference's type
/// arguments. Pure AST rewriting — no resolution — so it is safe to run while
/// structurally matching an `extends` pattern. Returns `None` when the name is
/// not a type alias in scope.
fn expand_named_alias_pattern(named: &ParsedNamedType, ctx: &CheckerContext) -> Option<ParsedType> {
    let body = match ctx.lookup_type_declaration(&named.name)? {
        TypeDeclarationInfo::Alias(info) => info.body.clone(),
        TypeDeclarationInfo::Interface(_) => return None,
    };

    let mut map: surge_ts_types::fx::FxHashMap<String, ParsedType> =
        surge_ts_types::fx::FxHashMap::default();
    for (index, parameter) in body.type_parameters.iter().enumerate() {
        if let Some(argument) = named.type_arguments.get(index) {
            map.insert(parameter.name.clone(), argument.clone());
        } else if let Some(default) = &parameter.default_type {
            map.insert(parameter.name.clone(), default.clone());
        }
    }

    Some(substitute_parsed_type_parameters_deep(&body.ty, &map))
}

/// Recursively rewrites bare named references in a parsed type using `map`,
/// recursing through every structural position (functions, unions, tuples,
/// objects, …). Unlike the shallower call-site helper, this reaches into function
/// parameter and return positions, which is required to push an `infer` capture
/// into an expanded alias body such as `JSXElementConstructor<infer P>`.
pub(crate) fn substitute_parsed_type_parameters_deep(
    ty: &ParsedType,
    map: &surge_ts_types::fx::FxHashMap<String, ParsedType>,
) -> ParsedType {
    match ty {
        ParsedType::Named(named) => {
            if named.type_arguments.is_empty() {
                if let Some(replacement) = map.get(&named.name) {
                    return replacement.clone();
                }
                return ParsedType::Named(named.clone());
            }
            ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
                name: named.name.clone(),
                span: named.span,
                type_arguments: named
                    .type_arguments
                    .iter()
                    .map(|argument| substitute_parsed_type_parameters_deep(argument, map))
                    .collect(),
            }))
        }
        ParsedType::Array(element) => ParsedType::Array(std::sync::Arc::new(
            substitute_parsed_type_parameters_deep(element, map),
        )),
        ParsedType::KeyOf(inner) => ParsedType::KeyOf(std::sync::Arc::new(
            substitute_parsed_type_parameters_deep(inner, map),
        )),
        ParsedType::Union(members) => ParsedType::Union(std::sync::Arc::new(
            members
                .iter()
                .map(|member| substitute_parsed_type_parameters_deep(member, map))
                .collect(),
        )),
        ParsedType::Intersection(members) => ParsedType::Intersection(std::sync::Arc::new(
            members
                .iter()
                .map(|member| substitute_parsed_type_parameters_deep(member, map))
                .collect(),
        )),
        ParsedType::Tuple(members) => ParsedType::Tuple(std::sync::Arc::new(
            members
                .iter()
                .map(|member| substitute_parsed_type_parameters_deep(member, map))
                .collect(),
        )),
        ParsedType::Function(function) => {
            let substituted = ParsedFunctionType {
                parameters: function
                    .parameters
                    .iter()
                    .map(|parameter| {
                        let mut parameter = parameter.clone();
                        parameter.ty = substitute_parsed_type_parameters_deep(&parameter.ty, map);
                        parameter
                    })
                    .collect(),
                return_type: Box::new(substitute_parsed_type_parameters_deep(
                    &function.return_type,
                    map,
                )),
                type_parameters: function.type_parameters.clone(),
            };
            ParsedType::Function(std::sync::Arc::new(substituted))
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod distributive_member_guard_tests {
    use crate::program::{SourceFileInput, check_program_with_stats_and_jobs};

    /// One file (no import) so the preliminary analysis round's import-less
    /// scope contributes no degraded resolution of its own, and the returned
    /// `h.value` forces the deferred `MakeRO<any>` conditional to actually
    /// distribute (an unused member never peels the lazy alias reference).
    fn forced_any_member_fixture() -> Vec<SourceFileInput> {
        vec![SourceFileInput {
            file_name: "single.ts".to_string(),
            source_text: "export type MakeRO<T> = T extends Map<infer K, infer V>\n\
                 \x20 ? ReadonlyMap<K, V>\n\
                 \x20 : Readonly<T>;\n\
                 export interface Holder<T> { value: MakeRO<T>; }\n\
                 export function go<T>(seed: T, h: Holder<any>): string {\n\
                 \x20 return h.value;\n\
                 }\n"
            .to_string(),
        }]
    }

    /// The pinning assertion for the member guards: distributing an `any`
    /// member must not select the `Map` branch with `K`/`V` unbound — before
    /// the guards, this exact fixture resolved `ReadonlyMap<K, V>` with both
    /// arguments unresolvable and finished with 1 degraded interface
    /// resolution out of 24 attempts (verified against the pre-fix binary);
    /// with the guards it is 0 of 13. The diagnostic surface alone cannot pin
    /// the failure (the lookup misses are suppressed in most windows), so the
    /// counter is the assertion.
    ///
    /// Known tsc divergence, deliberate: tsc types `MakeRO<any>` as the union
    /// of both branches (`Readonly<any> | ReadonlyMap<unknown, unknown>`) and
    /// reports TS2322 on the `string` return here; surge's `any` collapse
    /// (the same modeling the non-distributive path documents) under-reports
    /// this synthetic shape. On the real corpora the guards only removed
    /// diagnostics that the pinned tsc does not emit.
    #[test]
    fn any_member_produces_no_degraded_interface_resolutions() {
        // Safety: set before any checker thread is spawned in this test
        // process (nextest: one process per test); the counters gate is
        // re-derived from this env var at the start of every run.
        unsafe { std::env::set_var("SURGE_TIMINGS", "1") };
        let _ = check_program_with_stats_and_jobs(
            forced_any_member_fixture(),
            crate::CheckerOptions::default(),
            1,
        );
        let counters = crate::metrics::snapshot_program_counters();
        assert_eq!(
            counters.interface_resolution_degraded_count,
            0,
            "an `any` member must not degrade any interface resolution \
             (got {} degraded of {} attempts)",
            counters.interface_resolution_degraded_count,
            counters.interface_resolution_attempt_count,
        );
    }
}
