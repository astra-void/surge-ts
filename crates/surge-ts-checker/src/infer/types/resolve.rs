//! Core ParsedType -> Type resolution (tuples, functions, objects, unions, named, mapped).

use super::*;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedConditionalType, ParsedFunctionType, ParsedFunctionTypeParameter,
    ParsedIndexedAccessType, ParsedMappedType, ParsedNamedType, ParsedObjectType,
    ParsedTemplateLiteralType, ParsedType, ParsedTypeParameter, TextSpan,
};
use surge_ts_types::{
    NumberLiteralType, ObjectProperty, ObjectType, PropertyMap, Type, TypeCopyReason,
    is_assignable_to, union_type, with_type_copy_reason,
};

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::{CheckerContext, DeclarationResolutionKey, convert_span};
use crate::program::{
    record_generic_indexed_access_attempt, record_generic_indexed_access_invalid_key,
    record_generic_indexed_access_substituted_key,
    record_generic_indexed_access_substituted_receiver, record_generic_indexed_access_success,
    record_generic_indexed_access_unknown_fallback,
};
use crate::symbols::TypeDeclarationInfo;

pub(crate) fn resolve_parsed_type(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    match parsed_type {
        ParsedType::String => ResolvedType {
            ty: Type::String,
            had_error: false,
        },
        ParsedType::Number => ResolvedType {
            ty: Type::Number,
            had_error: false,
        },
        ParsedType::Boolean => ResolvedType {
            ty: Type::Boolean,
            had_error: false,
        },
        ParsedType::BigInt => ResolvedType {
            ty: Type::BigInt,
            had_error: false,
        },
        ParsedType::Symbol => ResolvedType {
            ty: Type::Symbol,
            had_error: false,
        },
        ParsedType::Undefined => ResolvedType {
            ty: Type::Undefined,
            had_error: false,
        },
        ParsedType::Any => ResolvedType {
            ty: Type::Any,
            had_error: false,
        },
        ParsedType::Unknown => ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        },
        ParsedType::UnknownKeyword => ResolvedType {
            ty: Type::GenuineUnknown,
            had_error: false,
        },
        ParsedType::Never => ResolvedType {
            ty: Type::Never,
            had_error: false,
        },
        ParsedType::StringLiteral(value) => ResolvedType {
            ty: Type::StringLiteral(value),
            had_error: false,
        },
        ParsedType::NumberLiteral(value) => ResolvedType {
            ty: Type::NumberLiteral(NumberLiteralType { value }),
            had_error: false,
        },
        ParsedType::BooleanLiteral(value) => ResolvedType {
            ty: Type::BooleanLiteral(value),
            had_error: false,
        },
        ParsedType::Void => ResolvedType {
            ty: Type::Void,
            had_error: false,
        },
        ParsedType::Object(object_type) => {
            resolve_object_type(object_type, ctx, resolving, substitution)
        }
        ParsedType::Array(element_type) => {
            let resolved_element = resolve_parsed_type(*element_type, ctx, resolving, substitution);
            ResolvedType {
                ty: Type::Array(Box::new(resolved_element.ty)),
                had_error: resolved_element.had_error,
            }
        }
        ParsedType::Tuple(elements) => resolve_tuple_type(elements, ctx, resolving, substitution),
        ParsedType::Union(types) => resolve_union_type(types, ctx, resolving, substitution),
        ParsedType::Intersection(types) => {
            resolve_intersection_type(types, ctx, resolving, substitution)
        }
        ParsedType::Function(function_type) => {
            resolve_function_type(function_type, ctx, resolving, substitution)
        }
        ParsedType::Named(named_type) => {
            resolve_named_type(named_type, ctx, resolving, substitution)
        }
        ParsedType::TypeOf(type_of) => {
            // `typeof X` references a value. During type-declaration resolution the
            // file's imported value bindings may not yet be in `ctx.symbols`, so on
            // a miss consult the module's full value table (the same forward-ref
            // fallback used when checking expressions); genuinely-missing names
            // still report TS2304.
            let symbol = ctx
                .symbols
                .get(&type_of.name)
                .cloned()
                .or_else(|| ctx.ambient_global_symbols.get(&type_of.name).cloned())
                .or_else(|| {
                    ctx.module_value_fallback
                        .as_ref()
                        .and_then(|table| table.get(&type_of.name).cloned())
                })
                .or_else(|| {
                    // `typeof X` inside an imported declaration's body is resolved
                    // under the declaring file's name (set by `with_file_name`), but
                    // the consumer's value `symbols`/`module_value_fallback` do not
                    // hold that module's locals. Consult the declaring module's own
                    // value table so a cross-module `Alias<typeof localConst>`
                    // resolves instead of falsely reporting TS2304.
                    let file_name = ctx.file_name.clone();
                    ctx.module_local_values_for_file(&file_name)
                        .and_then(|table| table.get(&type_of.name).cloned())
                });

            let Some(symbol) = symbol else {
                // `globalThis` is always a valid built-in, but its value symbol is
                // installed only after every ambient global is collected, so an
                // ambient declaration naming it (`declare var window: Window & typeof
                // globalThis`) resolves it first. Treat the miss as a clean `unknown`
                // (a false TS2304 / `had_error` would otherwise poison the enclosing
                // intersection); the `T & unknown ⇒ T` simplification then keeps
                // `window`/`self` as `Window`.
                if type_of.name == "globalThis" {
                    return ResolvedType {
                        ty: Type::Unknown,
                        had_error: false,
                    };
                }
                let mut diagnostic = Diagnostic::ts2304(&type_of.name, ctx.file_name.clone());
                if let Some(span) = type_of.name_span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);

                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                };
            };

            // `typeof NS.Root` walks the dotted member path off the base symbol's
            // type. Any segment we cannot model (a non-object base, or a missing
            // property on a namespace whose shape we don't fully reconstruct)
            // degrades to `Unknown` silently rather than emitting a false
            // positive, since the base name itself was resolved.
            let mut ty = symbol.ty;
            for member in &type_of.members {
                match ty.get_property_access_type(member) {
                    Some(member_ty) => ty = member_ty,
                    None => {
                        return ResolvedType {
                            ty: Type::Unknown,
                            had_error: false,
                        };
                    }
                }
            }

            ResolvedType {
                ty,
                had_error: false,
            }
        }
        ParsedType::KeyOf(inner) => {
            let resolved_inner = resolve_parsed_type(*inner, ctx, resolving, substitution);
            let mut keys = Vec::new();
            // Peel a nominal reference (`keyof User`) to read the named type's keys.
            match &resolved_inner.ty.peeled() {
                Type::Object(object_type) => {
                    for key in object_type.properties.keys() {
                        keys.push(Type::StringLiteral(key.clone()));
                    }
                }
                _ => {
                    return ResolvedType {
                        ty: Type::Unknown,
                        had_error: false,
                    };
                }
            }

            ResolvedType {
                ty: if keys.is_empty() {
                    Type::Unknown
                } else if keys.len() == 1 {
                    keys.into_iter().next().unwrap()
                } else {
                    union_type(keys)
                },
                had_error: false,
            }
        }
        ParsedType::Mapped(mapped) => resolve_mapped_type(mapped, ctx, resolving, substitution),
        ParsedType::IndexedAccess(indexed_access) => {
            resolve_indexed_access_type(indexed_access, ctx, resolving, substitution)
        }
        ParsedType::Conditional(conditional) => {
            resolve_conditional_type(conditional, ctx, resolving, substitution)
        }
        ParsedType::TemplateLiteral(template) => {
            resolve_template_literal_type(template, ctx, resolving, substitution)
        }
        // An `infer X` capture resolves to a permissive `any`: with no real
        // inference, the enclosing `extends` pattern (e.g. `Ctor<infer P>`) stays a
        // concrete shape so a non-matching check type correctly falls through to
        // the conditional's false branch, rather than collapsing to `unknown`
        // (which `is_assignable_to` would treat as matching). See
        // `resolve_conditional_type`.
        ParsedType::Infer(_) => ResolvedType {
            ty: Type::Any,
            had_error: false,
        },
    }
}

/// Maximum number of string-literal members a finite template expansion may
/// produce. Beyond this we fall back to broad `string` rather than materialise a
/// huge union (a defensive bound; real fixtures stay tiny).
const TEMPLATE_LITERAL_EXPANSION_LIMIT: usize = 10_000;

/// Evaluates a narrow subset of template literal types.
///
/// When every interpolation resolves to a finite set of string/number/boolean
/// literal members, the template expands to the cartesian product of its parts
/// as a deduped string-literal union (e.g. `` `/${"a"|"b"}/${"c"}` `` becomes
/// `"/a/c" | "/b/c"`). If any interpolation is a broad primitive (`string`,
/// `number`, …) or otherwise unresolved, the whole template degrades to broad
/// `string` so callers stay conservative and never cascade. This means a broad
/// template like `` `id:${string}` `` accepts any string — tsc is stricter, but
/// the mismatch is silent rather than a false positive.
pub(crate) fn resolve_template_literal_type(
    template: ParsedTemplateLiteralType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let ParsedTemplateLiteralType {
        quasis,
        interpolations,
        ..
    } = template;

    // The head is always present; interpolation i is followed by quasi i + 1.
    let mut combinations: Vec<String> = vec![quasis.first().cloned().unwrap_or_default()];
    let mut had_error = false;

    for (index, interpolation) in interpolations.into_iter().enumerate() {
        let resolved = resolve_parsed_type(interpolation, ctx, resolving, substitution);
        had_error |= resolved.had_error;

        let Some(parts) = finite_literal_strings(&resolved.ty) else {
            // Broad or unresolved interpolation: degrade to `string`.
            return ResolvedType {
                ty: Type::String,
                had_error,
            };
        };

        let suffix = quasis.get(index + 1).cloned().unwrap_or_default();
        if combinations.len().saturating_mul(parts.len()) > TEMPLATE_LITERAL_EXPANSION_LIMIT {
            return ResolvedType {
                ty: Type::String,
                had_error,
            };
        }

        let mut next = Vec::with_capacity(combinations.len() * parts.len());
        for prefix in &combinations {
            for part in &parts {
                next.push(format!("{prefix}{part}{suffix}"));
            }
        }
        combinations = next;
    }

    let members: Vec<Type> = combinations.into_iter().map(Type::StringLiteral).collect();

    ResolvedType {
        ty: union_type(members),
        had_error,
    }
}

/// Returns the finite set of literal string renderings for `ty` if it is a
/// string/number/boolean literal (or a union of such literals), or `None` if it
/// is a broad primitive or anything else that cannot be enumerated. Rendering
/// matches how each literal participates in a template literal: numbers by their
/// literal text and booleans as `true`/`false`.
fn finite_literal_strings(ty: &Type) -> Option<Vec<String>> {
    match ty {
        Type::StringLiteral(value) => Some(vec![value.clone()]),
        Type::NumberLiteral(value) => Some(vec![value.value.clone()]),
        Type::BooleanLiteral(value) => Some(vec![value.to_string()]),
        Type::Union(union) => {
            let mut parts = Vec::new();
            for member in union.types().iter() {
                parts.extend(finite_literal_strings(member)?);
            }
            Some(parts)
        }
        _ => None,
    }
}

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
        let members = match &resolved_check.ty {
            Type::Union(union) => union.types().to_vec(),
            Type::Never => Vec::new(),
            other => vec![other.clone()],
        };

        let mut results = Vec::new();
        let mut had_error = false;
        for member in members {
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
            let peeled = check.peeled();
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
    if callable_signature(&check.peeled()).is_none() {
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
fn substitute_parsed_type_parameters_deep(
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

pub(crate) fn resolve_tuple_type(
    elements: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_elements = Vec::new();
    let mut had_error = false;

    for element in elements {
        let resolved_element = resolve_parsed_type(element, ctx, resolving, substitution);
        had_error |= resolved_element.had_error;
        resolved_elements.push(resolved_element.ty);
    }

    ResolvedType {
        ty: Type::Tuple(resolved_elements),
        had_error,
    }
}

pub(crate) fn resolve_function_type(
    function_type: ParsedFunctionType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let local_substitution = extend_substitution_with_type_parameters(
        substitution,
        &function_type.type_parameters,
        ctx,
        resolving,
    );

    let value_parameters = function_type
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_this)
        .cloned()
        .collect::<Vec<_>>();
    let required_parameter_count = required_parameter_count(&value_parameters);
    let is_variadic = value_parameters
        .last()
        .is_some_and(|parameter| parameter.rest);
    let mut parameters = Vec::new();
    let mut had_error = false;

    for parameter in function_type.parameters.iter().cloned() {
        let is_this = parameter.is_this;
        let is_rest = parameter.rest;
        let resolved_parameter =
            resolve_function_type_parameter(parameter, ctx, resolving, &local_substitution);
        had_error |= resolved_parameter.had_error;
        // The `this` parameter is resolved so an unresolved `this` type still
        // reports once (and propagates `had_error` to avoid a cascade), but it is
        // not a real call parameter, so it is excluded from arity and arguments.
        if is_this {
            continue;
        }
        // A rest parameter is written as the array type but checked element-wise,
        // so store its element type to match variadic call/argument checking.
        if is_rest {
            parameters.push(rest_element_type(resolved_parameter.ty));
        } else {
            parameters.push(resolved_parameter.ty);
        }
    }

    let return_type = resolve_parsed_type(
        *function_type.return_type,
        ctx,
        resolving,
        &local_substitution,
    );
    had_error |= return_type.had_error;
    ResolvedType {
        ty: Type::Function(alloc_function_type(
            parameters,
            return_type.ty,
            is_variadic,
            required_parameter_count,
        )),
        had_error,
    }
}

fn rest_element_type(ty: Type) -> Type {
    match ty {
        Type::Array(element) => *element,
        other => other,
    }
}

pub(crate) fn required_parameter_count(
    parameters: &[surge_ts_syntax::ParsedFunctionTypeParameter],
) -> usize {
    let mut required = parameters.len();

    while required > 0 {
        let parameter = &parameters[required - 1];
        if parameter.optional || parameter.rest {
            required -= 1;
        } else {
            break;
        }
    }

    required
}

pub(crate) fn resolve_function_type_parameter(
    parameter: ParsedFunctionTypeParameter,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let ParsedFunctionTypeParameter { ty, .. } = parameter;
    let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
    ResolvedType {
        ty: resolved.ty,
        had_error: resolved.had_error,
    }
}

pub(crate) fn resolve_object_type(
    object_type: ParsedObjectType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = PropertyMap::new();
    let mut had_error = false;

    for property in object_type.properties {
        let property_type = resolve_parsed_type(property.ty, ctx, resolving, substitution);
        had_error |= property_type.had_error;
        let object_property = if property.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        };

        properties.insert(property.name, object_property);
    }

    let mut resolved_object = alloc_object_type(properties, None);
    if let Some(call_signature) = object_type.call_signature {
        let resolved = resolve_parsed_type(
            ParsedType::Function(*call_signature),
            ctx,
            resolving,
            substitution,
        );
        had_error |= resolved.had_error;
        if let Type::Function(function_type) = resolved.ty {
            resolved_object = resolved_object.with_call_signature(function_type);
        }
    }

    ResolvedType {
        ty: Type::Object(resolved_object),
        had_error,
    }
}

pub(crate) fn resolve_union_type(
    types: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_types = Vec::new();
    let mut had_error = false;

    for ty in types {
        let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
        had_error |= resolved.had_error;
        resolved_types.push(resolved.ty);
    }

    if resolved_types.is_empty() {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    ResolvedType {
        ty: union_type(resolved_types),
        had_error,
    }
}

/// Resolves an intersection `A & B`. Object-like operands are merged into a
/// single object exposing every member's property surface, which lets the
/// existing object machinery (property access, assignability, object-literal
/// checking) handle intersections without a dedicated runtime type. The merged
/// object is tagged via [`with_intersection_marker`] so a missing required
/// property surfaces the outer assignability code tsc reports for intersections.
///
/// Simplification follows the existing `any`/`unknown` policy: `T & any` is
/// `any`, `T & unknown` is `T`. Conflicting properties keep the left operand
/// (full `string & number -> never` reduction is a non-goal). If any operand is
/// unresolved the whole intersection degrades to `Unknown` after the root
/// diagnostic is reported, so reads stay conservative and never cascade.
pub(crate) fn resolve_intersection_type(
    types: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_types = Vec::new();
    let mut had_error = false;

    for ty in types {
        let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
        had_error |= resolved.had_error;
        resolved_types.push(resolved.ty);
    }

    // One failed operand must not erase the others: `ComponentProps<"button"> &
    // VariantProps<…>` with an unmodelled second operand still has a fully usable
    // first operand, and collapsing the whole intersection to `unknown` is what
    // strips contextual typing from every prop that flows through it. Merge the
    // usable members (the merge already drops `unknown` operands) but keep
    // `had_error` — the taint still gates every cache/bail exactly as before, so
    // no degraded shape is interned or re-expanded.
    if had_error {
        if resolved_types.iter().all(Type::is_unknown) {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }
        return ResolvedType {
            ty: merge_intersection_members(resolved_types),
            had_error: true,
        };
    }

    ResolvedType {
        ty: merge_intersection_members(resolved_types),
        had_error: false,
    }
}

fn merge_intersection_members(members: Vec<Type>) -> Type {
    if members.iter().any(|ty| matches!(ty, Type::Any)) {
        return Type::Any;
    }

    // A dropped `Type::Unknown` operand is surge's degradation sentinel for an
    // operand it could not model (`ComponentProps<typeof UnmodelledValue> & {…}`),
    // NOT the `unknown` keyword (`GenuineUnknown`). The failed operand may have
    // contributed members we never saw, so a surviving *inline object* surface
    // must stay OPEN — a closed merge would flag every dropped member's use as an
    // excess property. A surviving nominal reference is returned untouched (see
    // the lone-survivor comment below).
    let dropped_unmodelled_operand = members.iter().any(|ty| matches!(ty, Type::Unknown));
    let open_if_unmodelled = |ty: Type| -> Type {
        match ty {
            Type::Object(object) if dropped_unmodelled_operand && object.string_index_type.is_none() => {
                let mut object = object;
                object.string_index_type = Some(std::sync::Arc::new(Type::Any));
                Type::Object(object)
            }
            other => other,
        }
    };

    let members: Vec<Type> = members.into_iter().filter(|ty| !ty.is_unknown()).collect();

    // `T & unknown ⇒ T`: with the `unknown` operands dropped, a lone survivor is
    // returned unchanged. Peeling and re-merging it (below) would force a lazy
    // library reference's bounded structural expansion and discard its nominal
    // identity — e.g. `Window & typeof globalThis` would otherwise corrupt the
    // shared `Window` apparent type.
    if members.len() == 1 {
        return open_if_unmodelled(members.into_iter().next().unwrap());
    }

    let display_name = (!members.is_empty()).then(|| {
        members
            .iter()
            .map(Type::name)
            .collect::<Vec<_>>()
            .join(" & ")
    });

    // Peel reference operands (`StudentBulkImportRow & { … }`) so a named object
    // member contributes its properties to the merged intersection surface.
    let members: Vec<Type> = members.iter().map(Type::peeled).collect();

    let object_members: Vec<_> = members
        .iter()
        .filter_map(|ty| match ty {
            Type::Object(object) => Some(object),
            _ => None,
        })
        .collect();

    // Brand idiom: `string & { _?: never }` (and other `Base & {…all-optional…}`
    // shapes, e.g. `LiteralUnion<L, B> = L | (B & { _?: never })`). When every
    // object operand only contributes optional members, the object side is a
    // phantom "brand" and the intersection is structurally just the non-object
    // side — tsc treats `string & {}` as assignable both to and from `string`.
    // Collapsing to the non-object member keeps that bidirectional behavior;
    // falling through to the object-merge below would keep only `{ _?: never }`
    // and wrongly reject `(string & brand) → string`.
    if !object_members.is_empty()
        && object_members
            .iter()
            .all(|object| is_brand_like_object(object))
    {
        let mut non_object = members.iter().filter(|ty| !matches!(ty, Type::Object(_)));
        if let Some(first) = non_object.next() {
            if non_object.next().is_none() {
                return first.clone();
            }
        }
    }

    if !object_members.is_empty() {
        let mut properties: PropertyMap = PropertyMap::new();
        let mut string_index_type: Option<Type> = None;
        // A callable operand (`F & { … }`, or an interface with a call signature)
        // keeps the merged intersection callable; the first signature wins, like
        // conflicting properties.
        let mut call_signature = members.iter().find_map(|ty| match ty {
            Type::Function(function_type) => Some(std::sync::Arc::new(function_type.clone())),
            _ => None,
        });
        let mut construct_signature: Option<std::sync::Arc<surge_ts_types::FunctionType>> = None;

        for object in &object_members {
            for (name, property) in object.properties.iter() {
                properties
                    .entry(name.clone())
                    .or_insert_with(|| property.clone());
            }
            if string_index_type.is_none()
                && let Some(index) = object.string_index_type.as_deref()
            {
                string_index_type = Some(index.clone());
            }
            if call_signature.is_none() {
                call_signature = object.call_signature.clone();
            }
            if construct_signature.is_none() {
                construct_signature = object.construct_signature.clone();
            }
        }

        let mut merged =
            alloc_object_type(properties, string_index_type).with_intersection_marker();
        if let Some(display_name) = display_name {
            merged = merged.with_alias_name(display_name);
        }
        merged.call_signature = call_signature;
        merged.construct_signature = construct_signature;
        return open_if_unmodelled(Type::Object(merged));
    }

    match members.into_iter().next() {
        Some(member) => member,
        None => Type::Unknown,
    }
}

/// Whether an object contributes no required structure to an intersection — all
/// properties optional, no index signature, no call/construct signature. Such an
/// operand is a phantom "brand" (`{ _?: never }`), so `Base & brand` is
/// structurally just `Base`.
fn is_brand_like_object(object: &ObjectType) -> bool {
    object.string_index_type.is_none()
        && object.call_signature().is_none()
        && object.construct_signature().is_none()
        && object
            .properties
            .values()
            .all(|property| property.is_optional())
}

pub(crate) fn resolve_named_type(
    named_type: ParsedNamedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    if let Some(ty) = substitution.get(&named_type.name) {
        return ResolvedType {
            ty: ty.clone(),
            had_error: false,
        };
    }

    // Look up the declaration through a context-independent handle so resolution
    // can read the (often large) interface/alias payload while `ctx` is borrowed
    // mutably, without deep-cloning it. The handle keeps the backing arena alive;
    // the borrowed declaration below is decoupled from `ctx`.
    let Some(handle) = ctx.lookup_type_declaration_handle(&named_type.name) else {
        // A qualified reference (`React.Foo`, `Prisma.Bar`) we cannot resolve is
        // treated as no-cascade: tsc resolves these against the full namespace
        // surface and reports nothing, so emitting TS2304 here would be a false
        // positive against `@types/*` and generated namespace clients.
        if !named_type.name.contains('.') {
            emit_unknown_type_name(&named_type, ctx);
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };
    let declaration = handle.get();

    let has_type_arguments = !named_type.type_arguments.is_empty();
    let is_generic_declaration = match declaration {
        TypeDeclarationInfo::Alias(alias) => !alias.body.type_parameters.is_empty(),
        TypeDeclarationInfo::Interface(interface) => !interface.body.type_parameters.is_empty(),
    };

    if has_type_arguments && !is_generic_declaration {
        let name = match declaration {
            TypeDeclarationInfo::Alias(alias) => alias.name.as_str(),
            TypeDeclarationInfo::Interface(interface) => interface.name.as_str(),
        };
        emit_type_is_not_generic(name, named_type.span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    if !has_type_arguments && !is_generic_declaration {
        let cache_key = type_declaration_resolution_key(declaration);
        if let Some(cached) = get_cached_named_type_resolution(ctx, &cache_key, resolving) {
            return cached;
        }

        // Defer a library-scoped interface: its body (which transitively pulls the
        // mutually-recursive DOM/iterator graph) is expanded only when the
        // reference is peeled, so using the interface as a type argument no longer
        // collapses the enclosing instantiation. User interfaces and all type
        // aliases stay eager so their diagnostics and primitive/union expansions
        // are unchanged.
        if matches!(declaration, TypeDeclarationInfo::Interface(_))
            && declaration_file_is_library_scoped(declaration, ctx)
        {
            let alias_id = format!("{}\u{0}{}", cache_key.file_name, cache_key.name);
            let display = named_type.name.clone();
            let resolved = ResolvedType {
                ty: make_lazy_type_reference(
                    ctx,
                    &alias_id,
                    &display,
                    handle,
                    cache_key.clone(),
                    named_type.type_arguments,
                    Vec::new(),
                    substitution.clone_with_reason(TypeCopyReason::SubstitutionUnchanged),
                ),
                had_error: false,
            };
            cache_named_type_resolution(ctx, &cache_key, &resolved);
            return resolved;
        }

        mark_named_type_resolution_in_progress(ctx, &cache_key);
        let resolved = match declaration {
            TypeDeclarationInfo::Alias(alias) => resolve_type_alias(
                alias,
                handle.clone(),
                named_type.type_arguments,
                named_type.span,
                ctx,
                resolving,
                substitution,
                None,
            ),
            TypeDeclarationInfo::Interface(interface) => resolve_interface(
                interface,
                handle.clone(),
                named_type.type_arguments,
                ctx,
                resolving,
                substitution,
                None,
            ),
        };
        // tsc displays a non-generic interface/type-alias by its name in
        // diagnostics (e.g. `'StrictObj'`, not the structural expansion), and
        // treats it nominally: the qualified `file::name` identity lets
        // assignability recognise two resolutions of the same declaration.
        let alias_id = format!("{}\u{0}{}", cache_key.file_name, cache_key.name);
        let resolved = attach_object_alias_name(resolved, &named_type.name, &alias_id);
        // Wrap the named object in a lazy nominal reference. A non-generic
        // declaration is concrete and context-independent, so its expansion is
        // interned (the wrapped object keeps its `alias_id`/`alias_name`, so a
        // peeled reference still compares nominally and displays by name).
        let resolved =
            wrap_named_object_reference(resolved, &named_type.name, &alias_id, &cache_key, ctx);
        cache_named_type_resolution(ctx, &cache_key, &resolved);
        return resolved;
    }

    // A generic library/dependency instantiation is context-free once its type
    // arguments are fixed: its body binds against its own captured
    // `resolution_scope` and references only the global ambient surface. The real
    // lib typed-array/iterator cluster (`Uint8Array`, `ArrayIterator`,
    // `IteratorObject`, …) is mutually recursive and generic, so without memoizing
    // it every signature mentioning it re-expands the whole tree. Cache library
    // instantiations program-wide, keyed by the resolved type arguments. The
    // store is gated on the resolution being free of *external* cycles (see
    // `lowest_cycle_target_index`) so a cached value matches a standalone
    // resolution and never depends on what an enclosing frame had on the stack.
    let library_scoped = declaration_file_is_library_scoped(declaration, ctx);
    let library_cache_key = library_scoped.then(|| type_declaration_resolution_key(declaration));
    let decl_key = type_declaration_resolution_key(declaration);
    let reference_id = format!("{}\u{0}{}", decl_key.file_name, decl_key.name);
    // Resolve the type arguments once. The result is reused for the library cache
    // key, the nominal reference identity, AND — via `pre_resolved` below — the
    // authoritative `bind_type_arguments`, so a generic instantiation resolves its
    // arguments exactly once. Resolving them a second time in the authoritative
    // pass is exponential on deeply nested generics. Probe diagnostics are
    // discarded (`truncate_diagnostics` also releases the once-guard keys) so the
    // authoritative pass re-reports an unresolved argument rather than suppressing
    // it as a duplicate.
    let resolved_arguments: Option<Vec<Type>> = {
        let diagnostics_before = ctx.diagnostics().len();
        let mut arguments = Vec::with_capacity(named_type.type_arguments.len());
        let mut all_clean = true;
        for argument in &named_type.type_arguments {
            let resolved = resolve_parsed_type(argument.clone(), ctx, resolving, substitution);
            if resolved.had_error {
                all_clean = false;
                break;
            }
            arguments.push(resolved.ty);
        }
        ctx.truncate_diagnostics_releasing_utility_keys(diagnostics_before);
        all_clean.then_some(arguments)
    };
    let cached_arguments = if library_scoped {
        resolved_arguments.clone()
    } else {
        None
    };
    let reference_arguments = resolved_arguments;

    // tsc displays a generic instantiation by its alias form (`Box<string>`), not
    // the structural expansion. Build that display name from the resolved type
    // arguments and tag the resolved object with it for diagnostics.
    let alias_display_name =
        generic_instantiation_display_name(&named_type, declaration.declared_name());

    // An instantiation is only interned/short-circuited when no type parameters
    // are in scope. Inside a generic body, a placeholder argument (`Pick<T, K>`)
    // collapses to `unknown` when resolved, so two structurally-distinct
    // instantiations would share an interner key and a reused entry would return
    // the wrong type. Only a fully concrete instantiation is context-independent.
    // An instantiation is concrete when no type *parameters* are in scope. The
    // scope stack depth is not a proxy for this: a non-generic function body still
    // pushes an (empty) scope via `with_type_parameter_scope`, so checking
    // `is_empty()` would treat every instantiation built inside a plain function
    // body as non-concrete — eagerly expanding library generics (`new Uint8Array`)
    // into degraded structural objects (self-referential members collapse to
    // `unknown`) instead of nominal lazy references. Only a scope that actually
    // binds a parameter introduces placeholders, so gate on that.
    let concrete_instantiation = ctx
        .type_parameter_scopes
        .iter()
        .all(|scope| scope.is_empty());

    // Perf short-circuit: reuse a previously-interned instantiation with the same
    // resolved arguments without re-expanding the body. The interner holds only
    // diagnostic-free, cycle-independent, concrete expansions (see
    // `tag_generic_object_reference`), so a reused entry cannot drop a body
    // diagnostic — the hazard that makes a naive generic cache unsound.
    if concrete_instantiation
        && let (Some(display), Some(arguments)) =
            (alias_display_name.as_deref(), reference_arguments.as_ref())
        && let Some(entry) = lookup_instantiation(ctx, &decl_key, arguments)
    {
        return ResolvedType {
            ty: make_type_reference(
                reference_id.clone(),
                display.to_string(),
                arguments.clone(),
                entry.resolved,
            ),
            had_error: false,
        };
    }

    let generic_cache_key = cached_arguments.as_ref().and(library_cache_key);
    if let (Some(key), Some(arguments)) = (generic_cache_key.as_ref(), cached_arguments.as_ref()) {
        if let Some(hit) = get_persistent_generic_resolution(ctx, key, arguments) {
            return tag_generic_object_reference(
                hit,
                alias_display_name.as_deref(),
                &reference_id,
                &decl_key,
                reference_arguments.clone(),
                true,
                ctx,
            );
        }
    }

    // Defer a concrete library-scoped generic *interface* instantiation
    // (`HTMLAttributes<HTMLElement>`): expand its body only on peel so a use site
    // does not pull the whole DOM/iterator graph and collapse. Generic type
    // aliases stay eager (their bodies reference interfaces, which are themselves
    // deferred, so they stay bounded); non-concrete instantiations stay eager
    // because their placeholder substitution must not be frozen into a shared ref.
    if concrete_instantiation
        && matches!(declaration, TypeDeclarationInfo::Interface(_))
        && declaration_file_is_library_scoped(declaration, ctx)
        && let (Some(display), Some(arguments)) =
            (alias_display_name.as_ref(), reference_arguments.as_ref())
    {
        return ResolvedType {
            ty: make_lazy_type_reference(
                ctx,
                &reference_id,
                display,
                handle,
                decl_key.clone(),
                named_type.type_arguments,
                arguments.clone(),
                substitution.clone_with_reason(TypeCopyReason::SubstitutionUnchanged),
            ),
            had_error: false,
        };
    }

    // Measure cycles triggered by this resolution alone. The declaration is pushed
    // onto `resolving` (at index `floor`) inside `resolve_interface`/`resolve_type_alias`,
    // so a re-entry at `floor` or deeper is an internal self/mutual cycle that
    // resolves deterministically; a re-entry below `floor` reaches an outer frame.
    let floor = resolving.len();
    let saved_lowest_cycle = ctx.lowest_cycle_target_index;
    ctx.lowest_cycle_target_index = usize::MAX;
    // An instantiation is only safe to intern (and later short-circuit) if its body
    // resolution emitted no diagnostics: reusing one that emits would drop the
    // diagnostic. Track both the plain-diagnostic vector and the once-guard set.
    let diagnostics_before_body = ctx.diagnostics().len();
    let utility_keys_before_body = ctx.utility_diagnostic_keys.len();

    let resolved = match declaration {
        TypeDeclarationInfo::Alias(alias) => resolve_type_alias(
            alias,
            handle.clone(),
            named_type.type_arguments,
            named_type.span,
            ctx,
            resolving,
            substitution,
            reference_arguments.as_deref(),
        ),
        TypeDeclarationInfo::Interface(interface) => resolve_interface(
            interface,
            handle.clone(),
            named_type.type_arguments,
            ctx,
            resolving,
            substitution,
            reference_arguments.as_deref(),
        ),
    };

    let subtree_lowest_cycle = ctx.lowest_cycle_target_index;
    ctx.lowest_cycle_target_index = saved_lowest_cycle.min(subtree_lowest_cycle);

    let body_emitted_diagnostics = ctx.diagnostics().len() != diagnostics_before_body
        || ctx.utility_diagnostic_keys.len() != utility_keys_before_body;
    let cacheable = concrete_instantiation
        && subtree_lowest_cycle >= floor
        && !body_emitted_diagnostics
        && !resolved.had_error;

    if subtree_lowest_cycle >= floor {
        if let (Some(key), Some(arguments)) = (generic_cache_key, cached_arguments) {
            cache_persistent_generic_resolution(ctx, &key, arguments, &resolved);
        }
    }
    tag_generic_object_reference(
        resolved,
        alias_display_name.as_deref(),
        &reference_id,
        &decl_key,
        reference_arguments,
        cacheable,
        ctx,
    )
}

/// Wraps a successfully-resolved generic *object* instantiation in a
/// lazy/nominal [`Type::Reference`] over its interned structural expansion, so it
/// carries nominal identity (declaration + resolved arguments) and a `Box<T>`
/// display form without forcing re-expansion at later use sites. Non-object,
/// errored, argument-unresolved, or display-less resolutions fall back to the
/// previous structural object tagging.
fn tag_generic_object_reference(
    resolved: ResolvedType,
    display_name: Option<&str>,
    reference_id: &str,
    decl_key: &DeclarationResolutionKey,
    arguments: Option<Vec<Type>>,
    cacheable: bool,
    ctx: &CheckerContext,
) -> ResolvedType {
    // When the parsed arguments were not renderable (e.g. an object-literal type
    // argument), synthesize a display from the resolved argument types so the
    // instantiation still becomes a nominal `Type::Reference` carrying its
    // arguments. That representation is what conditional `infer` capture matches
    // against; without it an object-argument instantiation degraded to a bare
    // structural object and lost its arguments.
    let effective_display: Option<String> = match (display_name, &arguments) {
        (Some(display), _) => Some(display.to_string()),
        (None, Some(arguments)) if !arguments.is_empty() => Some(format!(
            "{}<{}>",
            decl_key.name,
            arguments
                .iter()
                .map(|argument| argument.name())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        _ => None,
    };
    match (effective_display.as_deref(), arguments, &resolved.ty) {
        (Some(display), Some(arguments), Type::Object(object)) if !resolved.had_error => {
            // Tag the structural object with the instantiation's display name so a
            // site that peels the reference (e.g. the TS2353 excess-property
            // message) still renders the nominal `Box<string>` form tsc uses,
            // rather than the structural expansion.
            let structural = Type::Object(object.clone().with_alias_name(display));
            // Only intern (making this instantiation reusable by the short-circuit)
            // when it is diagnostic-free and cycle-independent; otherwise keep a
            // private expansion so no other site reuses a context-dependent or
            // diagnostic-suppressing result.
            let interned = if cacheable {
                intern_instantiation(ctx, decl_key, &arguments, structural)
            } else {
                std::sync::Arc::new(structural)
            };
            ResolvedType {
                ty: make_type_reference(
                    reference_id.to_string(),
                    display.to_string(),
                    arguments,
                    interned,
                ),
                had_error: resolved.had_error,
            }
        }
        (display_name, _, _) => tag_generic_object_alias(resolved, display_name),
    }
}

/// Wraps a successfully-resolved *non-generic* named object (interface or type
/// alias) in a lazy nominal [`Type::Reference`] over its interned expansion. The
/// reference carries the declaration name for display and the qualified
/// `file\0name` identity for nominal equality. Non-object or errored resolutions
/// pass through unchanged so a `type Id = string` alias stays a plain `string`.
fn wrap_named_object_reference(
    resolved: ResolvedType,
    display: &str,
    reference_id: &str,
    decl_key: &DeclarationResolutionKey,
    ctx: &CheckerContext,
) -> ResolvedType {
    match &resolved.ty {
        Type::Object(_) if !resolved.had_error => {
            let interned = intern_instantiation(ctx, decl_key, &[], resolved.ty.clone());
            ResolvedType {
                ty: make_type_reference(
                    reference_id.to_string(),
                    display.to_string(),
                    Vec::new(),
                    interned,
                ),
                had_error: resolved.had_error,
            }
        }
        _ => resolved,
    }
}

fn declaration_file_is_library_scoped(
    declaration: &TypeDeclarationInfo,
    ctx: &CheckerContext,
) -> bool {
    let file_name = match declaration {
        TypeDeclarationInfo::Alias(alias) => alias.file_name.as_str(),
        TypeDeclarationInfo::Interface(interface) => interface.file_name.as_str(),
    };
    ctx.is_library_scoped_file(file_name)
}

/// Builds the alias display name for a generic instantiation (`Box<string>`)
/// from the *syntactic* type arguments. This renders arguments without resolving
/// them, so it has no diagnostic or caching side effects and — like tsc — keeps a
/// type-alias argument by its name rather than expanding it. Returns `None` when
/// there are no type arguments or any argument is not a simple renderable form.
fn generic_instantiation_display_name(
    named_type: &ParsedNamedType,
    declaration_name: &str,
) -> Option<String> {
    if named_type.type_arguments.is_empty() {
        return None;
    }

    let mut names = Vec::with_capacity(named_type.type_arguments.len());
    for argument in &named_type.type_arguments {
        names.push(crate::driver::parsed_type_display(argument)?);
    }

    Some(format!("{}<{}>", declaration_name, names.join(", ")))
}

/// Tags a successfully-resolved generic object instantiation with its alias
/// display name for diagnostics. Display-only: no `alias_id` is attached, so
/// nominal assignability is unchanged. Non-object, errored, or already-named
/// resolutions pass through unchanged.
fn tag_generic_object_alias(resolved: ResolvedType, display_name: Option<&str>) -> ResolvedType {
    match (display_name, &resolved.ty) {
        (Some(name), Type::Object(object))
            if !resolved.had_error && object.alias_name.is_none() =>
        {
            ResolvedType {
                ty: Type::Object(object.clone().with_alias_name(name)),
                had_error: resolved.had_error,
            }
        }
        _ => resolved,
    }
}

/// Tags a resolved object type with the interface/type-alias name it came from
/// so diagnostics display the name (tsc behaviour). Non-object resolutions and
/// errored resolutions pass through unchanged.
fn attach_object_alias_name(resolved: ResolvedType, name: &str, alias_id: &str) -> ResolvedType {
    match resolved.ty {
        // Tag the nominal identity even when the resolution errored (a cyclic
        // member may have collapsed to `unknown`): the object is still this named
        // declaration, so assignability can recognise two of its resolutions.
        Type::Object(object) => {
            let object = object.with_alias_id(alias_id);
            // tsc displays a named type by its name even when a deeply cyclic
            // member did not fully resolve (e.g. `URL`, whose `searchParams`
            // cluster is mutually recursive). Keep the display name whenever the
            // object resolved to a real shape; only a collapse to an empty object
            // (no recoverable structure) falls back to the structural form.
            let object = if resolved.had_error && object.properties.is_empty() {
                object
            } else {
                object.with_alias_name(name)
            };
            ResolvedType {
                ty: Type::Object(object),
                had_error: resolved.had_error,
            }
        }
        ty => ResolvedType {
            ty,
            had_error: resolved.had_error,
        },
    }
}

pub(crate) fn resolve_mapped_type(
    mapped: ParsedMappedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    // A homomorphic mapping (`[K in keyof X]`) preserves the source's
    // per-property optionality and its string index signature (tsc keeps both;
    // dropping the index signature turned `Flatten<T & Record<string, unknown>>`
    // members into spurious TS2339s). Capture the `keyof` operand so the source
    // shape is recoverable after the constraint is resolved to a key union.
    let keyof_operand: Option<ParsedType> = match mapped.constraint.as_ref() {
        ParsedType::KeyOf(inner) => Some(inner.as_ref().clone()),
        _ => None,
    };
    let resolved_constraint = resolve_parsed_type(*mapped.constraint, ctx, resolving, substitution);

    if resolved_constraint.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    // A `string` (non-literal) key constraint maps to a string index signature:
    // `{ [P in string]: T }` is `{ [k: string]: T }`. This is how `Record<string,
    // T>` resolves when it routes through its mapped-type body (physical libs)
    // rather than the built-in `resolve_record_utility_type` fast path. Without
    // this the mapped type collapsed to `unknown`, which surfaced as a spurious
    // missing-property error wherever the `Record` was a union member.
    if matches!(resolved_constraint.ty, Type::String) {
        let mut value_substitution =
            substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        value_substitution.insert(mapped.key_name.clone(), Type::String);
        let resolved_value =
            resolve_parsed_type(*mapped.value_type, ctx, resolving, &value_substitution);
        return ResolvedType {
            ty: Type::Object(alloc_object_type(
                PropertyMap::new(),
                Some(resolved_value.ty),
            )),
            had_error: resolved_value.had_error,
        };
    }

    let keys = match resolved_constraint.ty {
        Type::StringLiteral(s) => vec![s],
        Type::Union(union) => {
            let mut keys = Vec::new();
            for variant in union.types() {
                match variant {
                    Type::StringLiteral(s) => keys.push(s.clone()),
                    _ => {
                        return ResolvedType {
                            ty: Type::Unknown,
                            had_error: false,
                        };
                    }
                }
            }
            keys
        }
        _ => {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: false,
            };
        }
    };

    let homomorphic_source = keyof_operand.and_then(|operand| {
        let resolved = resolve_parsed_type(operand, ctx, resolving, substitution);
        match resolved.ty.peeled() {
            Type::Object(object) => Some(object),
            _ => None,
        }
    });

    let mut properties = PropertyMap::new();
    let mut had_error = false;

    for key in keys {
        let mut new_substitution =
            substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        new_substitution.insert(mapped.key_name.clone(), Type::StringLiteral(key.clone()));

        let resolved_value = resolve_parsed_type(
            *mapped.value_type.clone(),
            ctx,
            resolving,
            &new_substitution,
        );

        if resolved_value.had_error {
            had_error = true;
        }

        let source_optional = homomorphic_source
            .as_ref()
            .and_then(|object| object.get_property(&key))
            .is_some_and(|property| property.is_optional());
        properties.insert(
            key,
            ObjectProperty {
                ty: resolved_value.ty,
                optional: mapped.optional || source_optional,
            },
        );
    }

    // Reusing the source's index value type is exact for identity mappings
    // (`T[k]`) and an approximation for transforming ones; either way it keeps
    // index-signature reads legal, matching tsc's homomorphic behaviour.
    let index_type = homomorphic_source
        .as_ref()
        .and_then(|object| object.string_index_type.as_deref().cloned());

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, index_type)),
        had_error,
    }
}

pub(crate) fn resolve_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        resolve_parsed_type(parsed_type, ctx, resolving, substitution)
    })
}

pub(crate) fn bind_type_arguments(
    type_parameters: &[ParsedTypeParameter],
    type_arguments: Vec<ParsedType>,
    name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    parent_substitution: &TypeParameterSubstitution,
    pre_resolved: Option<&[Type]>,
) -> Option<TypeParameterSubstitution> {
    if type_parameters.is_empty() {
        if !type_arguments.is_empty() {
            emit_type_is_not_generic(name, name_span, ctx);
            return None;
        }

        return Some(TypeParameterSubstitution::new());
    }

    if type_arguments.len() > type_parameters.len() {
        emit_generic_arity(name, type_parameters.len(), name_span, ctx);
        return None;
    }

    let mut substitution = TypeParameterSubstitution::new();

    for (index, parameter) in type_parameters.iter().enumerate() {
        if let Some(argument) = type_arguments.get(index) {
            // Reuse the caller's already-resolved argument when available instead
            // of resolving the `ParsedType` a second time. The redundant
            // resolution is exponential on deeply nested generics (each level
            // re-resolves its arguments), so reusing the probe result is what keeps
            // a nominal-reference instantiation linear.
            let resolved_ty = if let Some(pre) = pre_resolved.and_then(|pre| pre.get(index)) {
                pre.clone()
            } else {
                let resolved_argument =
                    resolve_parsed_type(argument.clone(), ctx, resolving, parent_substitution);
                if resolved_argument.had_error {
                    return None;
                }
                resolved_argument.ty
            };

            if parsed_type_is_placeholder_reference(argument, parent_substitution) {
                substitution.insert_placeholder(parameter.name.clone(), resolved_ty);
            } else {
                substitution.insert(parameter.name.clone(), resolved_ty);
            }
            continue;
        }

        let Some(default_type) = parameter.default_type.clone() else {
            emit_generic_arity(name, type_parameters.len(), name_span, ctx);
            return None;
        };

        let mut effective_substitution =
            parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        effective_substitution
            .extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged));

        let default_type_is_placeholder =
            parsed_type_is_placeholder_reference(&default_type, &effective_substitution);
        let resolved_default =
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution);
        if resolved_default.had_error {
            return None;
        }

        if default_type_is_placeholder {
            substitution.insert_placeholder(parameter.name.clone(), resolved_default.ty);
        } else {
            substitution.insert(parameter.name.clone(), resolved_default.ty);
        }
    }

    Some(substitution)
}

pub(crate) fn extend_substitution_with_type_parameters(
    parent_substitution: &TypeParameterSubstitution,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
) -> TypeParameterSubstitution {
    let mut substitution =
        parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);

    for parameter in type_parameters {
        let mut effective_substitution =
            parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        effective_substitution
            .extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged));

        let resolved = parameter.default_type.clone().map(|default_type| {
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution)
        });

        let ty = match resolved {
            Some(resolved) if !resolved.had_error => resolved.ty,
            Some(_) => Type::Unknown,
            None => Type::Unknown,
        };

        if let Some(default_type) = parameter.default_type.as_ref() {
            if parsed_type_is_placeholder_reference(default_type, &effective_substitution) {
                substitution.insert_placeholder(parameter.name.clone(), ty);
                continue;
            }
        }

        substitution.insert(parameter.name.clone(), ty);
    }

    substitution
}

pub(crate) fn parsed_type_is_placeholder_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    matches!(
        parsed_type,
        ParsedType::Named(named_type) if substitution.is_placeholder(&named_type.name)
    )
}

pub(crate) fn parsed_type_placeholder_name<'a>(
    parsed_type: &'a ParsedType,
    substitution: &TypeParameterSubstitution,
) -> Option<&'a str> {
    match parsed_type {
        ParsedType::Named(named_type) if substitution.is_placeholder(&named_type.name) => {
            Some(named_type.name.as_str())
        }
        _ => None,
    }
}

trait ParsedTypeSpan {
    fn span(&self) -> Option<TextSpan>;
}

impl ParsedTypeSpan for ParsedType {
    fn span(&self) -> Option<TextSpan> {
        match self {
            ParsedType::Named(named_type) => named_type.span,
            ParsedType::TypeOf(type_of) => type_of.name_span,
            ParsedType::IndexedAccess(indexed_access) => indexed_access.span,
            ParsedType::Mapped(mapped) => mapped.key_span.or(mapped.span),
            _ => None,
        }
    }
}

pub(crate) fn is_concrete_substituted_named_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    matches!(
        parsed_type,
        ParsedType::Named(named_type)
            if substitution
                .get(&named_type.name)
                .is_some()
                && !substitution.is_placeholder(&named_type.name)
    )
}

pub(crate) fn is_concrete_substituted_index_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    match parsed_type {
        ParsedType::Named(named_type) => {
            substitution.get(&named_type.name).is_some()
                && !substitution.is_placeholder(&named_type.name)
        }
        ParsedType::KeyOf(inner) => {
            is_concrete_substituted_named_reference(inner.as_ref(), substitution)
        }
        _ => false,
    }
}

/// Selects the type of `index` from `object` without emitting diagnostics or
/// recording cascade errors. Used when the receiver already errored but its
/// structural shape is still usable, so the requested property can be selected
/// without cascading a fresh missing-property diagnostic. Returns `None` when
/// the receiver is not an indexable structure or the key is not present.
fn select_indexed_property_no_cascade(object: &Type, index: &Type) -> Option<Type> {
    match (object, index) {
        (Type::Object(object_type), Type::StringLiteral(key)) => {
            object_type.get_property_access_type(key)
        }
        (Type::Object(object_type), Type::Union(union_ty)) => {
            let mut types = Vec::new();
            for key_ty in union_ty.types() {
                let Type::StringLiteral(key) = key_ty else {
                    return None;
                };
                types.push(object_type.get_property_access_type(key)?);
            }
            Some(union_type(types))
        }
        (Type::Tuple(elements), Type::NumberLiteral(num)) => {
            let index = num.value.parse::<usize>().ok()?;
            elements.get(index).cloned()
        }
        (Type::Array(element_type), Type::Number) => Some(*element_type.clone()),
        (Type::Tuple(elements), Type::Number) => Some(union_type(elements.clone())),
        _ => None,
    }
}

fn resolve_indexed_access_type(
    indexed_access: ParsedIndexedAccessType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    record_generic_indexed_access_attempt();
    let object_type_for_placeholder = indexed_access.object_type.clone();
    let object_placeholder_name =
        parsed_type_placeholder_name(object_type_for_placeholder.as_ref(), substitution);
    let index_placeholder_name =
        parsed_type_placeholder_name(indexed_access.index_type.as_ref(), substitution);
    let object_is_concrete_substitution =
        is_concrete_substituted_named_reference(object_type_for_placeholder.as_ref(), substitution);
    let index_is_concrete_substitution =
        is_concrete_substituted_index_reference(indexed_access.index_type.as_ref(), substitution);
    let generic_indexed_access = object_placeholder_name.is_some()
        || index_placeholder_name.is_some()
        || object_is_concrete_substitution
        || index_is_concrete_substitution;
    let index_is_keyof_same_placeholder = matches!(
        (
            object_placeholder_name.as_deref(),
            indexed_access.index_type.as_ref()
        ),
        (
            Some(object_name),
            ParsedType::KeyOf(inner)
        ) if matches!(
            inner.as_ref(),
            ParsedType::Named(named_type) if named_type.name == object_name
        )
    );
    // `K extends keyof T` makes the generic `T[K]` a valid index even though
    // neither side is concrete yet, so it must not cascade into TS2536.
    let index_constraint_satisfies_object = match (
        object_placeholder_name.as_deref(),
        index_placeholder_name.as_deref(),
    ) {
        (Some(object_name), Some(index_name)) => {
            ctx.type_parameter_keyof_constraint_target(index_name) == Some(object_name)
        }
        _ => false,
    };
    let index_is_valid_generic_key =
        index_is_keyof_same_placeholder || index_constraint_satisfies_object;

    // An index access through a *constrained* type parameter (`T extends …`,
    // `K extends Key`, `strict extends Boolean`, …) is validated by tsc against
    // that constraint. We do not fully resolve those (often library-generated)
    // constraints, so verifying the key here would only ever produce false
    // `TS2536`/`TS2538`s. An unconstrained `T[K]` is still a genuine error and
    // is left to the checks below.
    let involves_constrained_type_parameter = object_placeholder_name
        .as_deref()
        .is_some_and(|name| ctx.type_parameter_has_constraint(name))
        || index_placeholder_name
            .as_deref()
            .is_some_and(|name| ctx.type_parameter_has_constraint(name));

    if object_is_concrete_substitution {
        record_generic_indexed_access_substituted_receiver();
    }
    if index_is_concrete_substitution {
        record_generic_indexed_access_substituted_key();
    }

    let resolved_object =
        resolve_parsed_type(*indexed_access.object_type, ctx, resolving, substitution);
    // Peel a nominal reference receiver (`User["id"]`) to its structural object so
    // the index lookup below reads its properties instead of failing to match.
    let resolved_object = ResolvedType {
        ty: resolved_object.ty.peeled(),
        had_error: resolved_object.had_error,
    };

    let resolved_index = resolve_parsed_type(
        *indexed_access.index_type.clone(),
        ctx,
        resolving,
        substitution,
    );

    if resolved_object.had_error {
        if generic_indexed_access {
            record_generic_indexed_access_unknown_fallback();
        }
        // The receiver shape is known but one of its inner property types could
        // not be resolved (e.g. an imported alias whose body references a lib
        // type unavailable in the declaring module's scope). Still select the
        // requested property so downstream code sees the right type, without
        // emitting a fresh diagnostic from a receiver that already errored. The
        // selected property is a legitimate type, so it is returned clean and
        // participates normally in narrowing. A truly unresolved receiver
        // (Unknown) has no selectable property and stays no-cascade as Unknown.
        if let Some(selected) =
            select_indexed_property_no_cascade(&resolved_object.ty, &resolved_index.ty)
        {
            return ResolvedType {
                ty: selected,
                had_error: false,
            };
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    if (object_placeholder_name.is_some() && index_is_valid_generic_key)
        || involves_constrained_type_parameter
    {
        if generic_indexed_access {
            record_generic_indexed_access_unknown_fallback();
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    // A receiver that *resolved* to `unknown` (e.g. `typeof external` whose
    // value type could not be reconstructed, or a generic alias whose body we do
    // not model) cannot have its index validated, so indexing it degrades to
    // `unknown` rather than a false `TS2536`/`TS2538`. Excluded:
    // - a naked type-parameter receiver (`object_placeholder`): unconstrained
    //   `T[K]` is a genuine error handled below;
    // - an *explicit* `unknown`/`any` keyword receiver (`unknown["x"]`): tsc does
    //   report `TS2339`/`TS2538` there, so it must not be suppressed.
    let object_is_explicit_top_keyword = matches!(
        object_type_for_placeholder.as_ref(),
        ParsedType::Unknown | ParsedType::UnknownKeyword | ParsedType::Any
    );
    if resolved_object.ty.is_unknown()
        && object_placeholder_name.is_none()
        && !object_is_explicit_top_keyword
    {
        if generic_indexed_access {
            record_generic_indexed_access_unknown_fallback();
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    if !index_is_valid_generic_key
        && (index_placeholder_name.is_some() || object_placeholder_name.is_some())
    {
        let index_name = index_placeholder_name
            .map(str::to_string)
            .unwrap_or_else(|| resolved_index.ty.name());
        let object_name = object_placeholder_name
            .map(str::to_string)
            .unwrap_or_else(|| resolved_object.ty.name());
        let mut diagnostic = Diagnostic::ts2536(&index_name, &object_name, ctx.file_name.clone());
        if let Some(span) = indexed_access
            .index_type
            .as_ref()
            .span()
            .or(indexed_access.span)
        {
            diagnostic = diagnostic.with_span(convert_span(span));
        }
        ctx.push(diagnostic);
        if generic_indexed_access {
            record_generic_indexed_access_invalid_key();
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    match (&resolved_object.ty, &resolved_index.ty) {
        (Type::Object(object_type), Type::StringLiteral(key)) => {
            if let Some(property_ty) = object_type.get_property_access_type(&key) {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                ResolvedType {
                    ty: property_ty,
                    had_error: false,
                }
            } else {
                let mut diagnostic =
                    Diagnostic::ts2339(key, &resolved_object.ty.name(), ctx.file_name.clone());
                if let Some(span) = indexed_access.span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            }
        }
        // A numeric-literal index keys an object by its stringified value:
        // `{ 0: 1; 1: 0 }[0]` reads the `"0"` property. Object literals with
        // numeric keys are common in library-generated conditional-type tables
        // (e.g. Prisma's `{ 0: …; 1: … }[B]`).
        (Type::Object(object_type), Type::NumberLiteral(num)) => {
            if let Some(property_ty) = object_type.get_property_access_type(&num.value) {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                ResolvedType {
                    ty: property_ty,
                    had_error: false,
                }
            } else {
                let mut diagnostic = Diagnostic::ts2339(
                    &num.value,
                    &resolved_object.ty.name(),
                    ctx.file_name.clone(),
                );
                if let Some(span) = indexed_access.span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            }
        }
        (Type::Object(object_type), Type::Union(union_ty)) => {
            let mut types = Vec::new();
            let mut had_error = false;
            for key_ty in union_ty.types() {
                let key = match key_ty {
                    Type::StringLiteral(key) => Some(key.clone()),
                    Type::NumberLiteral(num) => Some(num.value.clone()),
                    _ => None,
                };
                if let Some(key) = key {
                    if let Some(property_ty) = object_type.get_property_access_type(&key) {
                        types.push(property_ty);
                    } else {
                        let mut diagnostic = Diagnostic::ts2339(
                            &key,
                            &resolved_object.ty.name(),
                            ctx.file_name.clone(),
                        );
                        if let Some(span) = indexed_access.span {
                            diagnostic = diagnostic.with_span(convert_span(span));
                        }
                        ctx.push(diagnostic);
                        had_error = true;
                    }
                } else {
                    let mut diagnostic = Diagnostic::ts2538(&key_ty.name(), ctx.file_name.clone());
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                    had_error = true;
                }
            }

            if had_error {
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            } else {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                ResolvedType {
                    ty: union_type(types),
                    had_error: false,
                }
            }
        }
        (Type::Tuple(elements), Type::NumberLiteral(num)) => {
            if let Ok(index) = num.value.parse::<usize>() {
                if let Some(element_ty) = elements.get(index) {
                    if generic_indexed_access {
                        record_generic_indexed_access_success();
                    }
                    ResolvedType {
                        ty: element_ty.clone(),
                        had_error: false,
                    }
                } else {
                    let mut diagnostic = Diagnostic::ts2493(
                        &resolved_object.ty.name(),
                        elements.len(),
                        index,
                        ctx.file_name.clone(),
                    );
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                    ResolvedType {
                        ty: Type::Unknown,
                        had_error: true,
                    }
                }
            } else {
                ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                }
            }
        }
        (Type::Array(element_type), Type::Number) => ResolvedType {
            ty: {
                if generic_indexed_access {
                    record_generic_indexed_access_success();
                }
                *element_type.clone()
            },
            had_error: false,
        },
        (Type::Tuple(elements), Type::Number) => {
            if generic_indexed_access {
                record_generic_indexed_access_success();
            }
            ResolvedType {
                ty: union_type(elements.clone()),
                had_error: false,
            }
        }
        (Type::Any, _) | (_, Type::Any) => {
            if generic_indexed_access {
                record_generic_indexed_access_success();
            }
            ResolvedType {
                ty: Type::Any,
                had_error: false,
            }
        }
        (_, Type::StringLiteral(key)) => {
            let mut diagnostic =
                Diagnostic::ts2339(key, &resolved_object.ty.name(), ctx.file_name.clone());
            if let Some(span) = indexed_access.span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push(diagnostic);
            if generic_indexed_access {
                record_generic_indexed_access_unknown_fallback();
            }
            ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            }
        }
        (_, invalid_index) => {
            if let Type::Unknown = invalid_index {
                // In a generic context (a type-parameter receiver/key or a
                // substituted reference) an `unknown` index is a resolution
                // limitation we cannot validate — e.g. `T[keyof T]` where `keyof T`
                // could not be computed — not the literal `value[unknownKey]` that
                // tsc flags. Degrade silently rather than emit a false TS2538.
                if !generic_indexed_access
                    && ctx.options.diagnostic_profile != crate::context::DiagnosticProfile::Native
                {
                    let mut diagnostic =
                        Diagnostic::ts2538(&invalid_index.name(), ctx.file_name.clone());
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                }
                if generic_indexed_access {
                    record_generic_indexed_access_unknown_fallback();
                }
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: !generic_indexed_access,
                };
            }
            let mut diagnostic = Diagnostic::ts2538(&invalid_index.name(), ctx.file_name.clone());
            if let Some(span) = indexed_access.span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push(diagnostic);
            if generic_indexed_access {
                record_generic_indexed_access_unknown_fallback();
            }
            ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            }
        }
    }
}
