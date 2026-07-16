use super::*;

use surge_ts_syntax::{ParsedConditionalType, ParsedNamedType};
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

    let resolved_extends =
        resolve_parsed_type(*conditional.extends_type, ctx, resolving, substitution);
    // Only bail when the extends pattern is structureless: a usable shape that
    // merely tainted `had_error` from an unmodelled deep member (e.g. React's
    // `JSXElementConstructor<P>`, whose body pulls `ReactNode`/`Component`) is
    // still enough to decide the branch assignability test, so the conditional
    // must proceed rather than collapse — that collapse is what blocked
    // `ComponentProps<"input">` from selecting its `JSX.IntrinsicElements[T]` branch.
    if resolved_extends.had_error && resolved_extends.ty.is_unknown() {
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
            if matches!(member, Type::Unknown) {
                results.push(Type::Unknown);
                continue;
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
            let branch = if !resolved_extends.ty.is_unknown()
                && is_assignable_to(&member, &resolved_extends.ty)
            {
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
            };

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

    // Non-distributive: only evaluate when the check type is concrete enough for a
    // meaningful assignability test. An unresolved generic parameter resolves to
    // `Unknown`, which we treat as "cannot decide" and degrade.
    if resolved_check.ty.is_unknown() || resolved_extends.ty.is_unknown() {
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
    ctx: &CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    depth: usize,
    reference_positional: bool,
) {
    match extends {
        ParsedType::Infer(name) => {
            substitution.insert(name.clone(), check.clone());
        }
        ParsedType::Array(element) => {
            if let Type::Array(check_element) = check {
                bind_infer_captures(
                    element,
                    check_element,
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
            if let Some(check_function) = callable_signature(&peeled) {
                let check_parameters = check_function.parameters();
                let pattern_parameters = pattern
                    .parameters
                    .iter()
                    .filter(|parameter| !parameter.is_this);
                for (index, pattern_parameter) in pattern_parameters.enumerate() {
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
        }
        // A union/intersection extends pattern (e.g. the body of
        // `JSXElementConstructor`) binds from whichever member structurally lines
        // up with the check type; members that do not align bind nothing.
        ParsedType::Union(members) | ParsedType::Intersection(members) => {
            for member in members {
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
            if reference_positional && let Type::Reference(reference) = check {
                for (pattern_argument, check_argument) in
                    named.type_arguments.iter().zip(reference.arguments.iter())
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
    ctx: &CheckerContext,
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

/// The callable signature of a check type, treating a function and a callable
/// object (one carrying a call or construct signature, e.g. React's
/// `ForwardRefExoticComponent<P>` or a class value) uniformly. This lets
/// `JSXElementConstructor<infer P>` recover the props type from a `forwardRef`/
/// `memo` component, not only from a plain function component.
fn callable_signature(ty: &Type) -> Option<&surge_ts_types::FunctionType> {
    match ty {
        Type::Function(function) => Some(function),
        Type::Object(object) => object
            .call_signature()
            .or_else(|| object.construct_signature()),
        _ => None,
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
            for member in members {
                collect_infer_names(member, names);
            }
        }
        ParsedType::Function(function) => {
            for parameter in &function.parameters {
                collect_infer_names(&parameter.ty, names);
            }
            collect_infer_names(&function.return_type, names);
        }
        ParsedType::Named(named) => {
            for argument in &named.type_arguments {
                collect_infer_names(argument, names);
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
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => parsed_type_contains_infer(inner),
        ParsedType::Union(members)
        | ParsedType::Intersection(members)
        | ParsedType::Tuple(members) => members.iter().any(parsed_type_contains_infer),
        ParsedType::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| parsed_type_contains_infer(&parameter.ty))
                || parsed_type_contains_infer(&function.return_type)
        }
        ParsedType::Named(named) => named.type_arguments.iter().any(parsed_type_contains_infer),
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

    let mut map: std::collections::HashMap<String, ParsedType> = std::collections::HashMap::new();
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
    map: &std::collections::HashMap<String, ParsedType>,
) -> ParsedType {
    match ty {
        ParsedType::Named(named) => {
            if named.type_arguments.is_empty() {
                if let Some(replacement) = map.get(&named.name) {
                    return replacement.clone();
                }
                return ParsedType::Named(named.clone());
            }
            let mut substituted = named.clone();
            substituted.type_arguments = named
                .type_arguments
                .iter()
                .map(|argument| substitute_parsed_type_parameters_deep(argument, map))
                .collect();
            ParsedType::Named(substituted)
        }
        ParsedType::Array(element) => ParsedType::Array(Box::new(
            substitute_parsed_type_parameters_deep(element, map),
        )),
        ParsedType::KeyOf(inner) => {
            ParsedType::KeyOf(Box::new(substitute_parsed_type_parameters_deep(inner, map)))
        }
        ParsedType::Union(members) => ParsedType::Union(
            members
                .iter()
                .map(|member| substitute_parsed_type_parameters_deep(member, map))
                .collect(),
        ),
        ParsedType::Intersection(members) => ParsedType::Intersection(
            members
                .iter()
                .map(|member| substitute_parsed_type_parameters_deep(member, map))
                .collect(),
        ),
        ParsedType::Tuple(members) => ParsedType::Tuple(
            members
                .iter()
                .map(|member| substitute_parsed_type_parameters_deep(member, map))
                .collect(),
        ),
        ParsedType::Function(function) => {
            let mut substituted = function.clone();
            substituted.parameters = function
                .parameters
                .iter()
                .map(|parameter| {
                    let mut parameter = parameter.clone();
                    parameter.ty = substitute_parsed_type_parameters_deep(&parameter.ty, map);
                    parameter
                })
                .collect();
            substituted.return_type = Box::new(substitute_parsed_type_parameters_deep(
                &function.return_type,
                map,
            ));
            ParsedType::Function(substituted)
        }
        other => other.clone(),
    }
}
