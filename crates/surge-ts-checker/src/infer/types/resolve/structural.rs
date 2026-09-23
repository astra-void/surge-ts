use super::*;

use surge_ts_syntax::{
    ParsedFunctionType, ParsedFunctionTypeParameter, ParsedObjectType, ParsedTupleElement,
};
use surge_ts_types::{ObjectProperty, PropertyMap};

use crate::metrics::{alloc_function_type, alloc_object_type};

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

/// A tuple written with a spread element (`[...a, ...b]`, `[head, ...tail]`).
/// When every spread operand resolves to a known-length tuple the whole tuple
/// has a known length, so the operands splice in place and the result is an
/// ordinary fixed tuple — which is what makes `[...path, 0]` and the
/// `[head, ...tail]` list idiom evaluate instead of degrading. A length-less
/// operand (an array, an unresolved parameter) leaves the length unknown; the
/// fixed-length model cannot state that, so it degrades as it did before this
/// shape was modelled at all.
/// `SURGE_READONLY_ARRAYS=0` collapses `ReadonlyArray<T>` to the mutable array
/// again; the parser reads the same switch for the `readonly` operator.
pub(crate) fn readonly_arrays_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_READONLY_ARRAYS").as_deref() != Ok("0"))
}

struct ReadonlyShape(Type);

impl surge_ts_types::ResolveReference for ReadonlyShape {
    fn resolve(&self) -> Type {
        self.0.clone()
    }
}

/// Wraps an array or tuple shape in the nominal `readonly` reference. Anything
/// else (a degraded operand, a union the operator was written over in error)
/// is returned unchanged so no new degradation is introduced.
pub(crate) fn readonly_reference(ty: Type) -> Type {
    match ty {
        Type::Reference(ref reference) if reference.is_readonly_array() => ty,
        Type::Array(_) | Type::Tuple(_) | Type::OpenTuple(_) => {
            let display = format!("readonly {}", ty.name());
            Type::Reference(surge_ts_types::TypeReference::new(
                surge_ts_types::READONLY_REFERENCE_ID,
                display,
                vec![ty.clone()],
                std::sync::Arc::new(ReadonlyShape(ty)),
            ))
        }
        other => other,
    }
}

pub(crate) fn resolve_variadic_tuple_type(
    elements: Vec<ParsedTupleElement>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved = Vec::with_capacity(elements.len());
    let mut had_error = false;

    for element in elements {
        let (is_rest, written) = match element {
            ParsedTupleElement::Fixed(written) => (false, written),
            ParsedTupleElement::Rest(written) => (true, written),
        };
        let resolved_element = resolve_parsed_type(written, ctx, resolving, substitution);
        had_error |= resolved_element.had_error;
        resolved.push((is_rest, resolved_element.ty));
    }

    if let Some(members) = splice_spread_operands(&resolved) {
        return ResolvedType {
            ty: Type::Tuple(members),
            had_error,
        };
    }
    ResolvedType {
        ty: open_tuple_from_operands(&resolved).unwrap_or(Type::Unknown),
        had_error,
    }
}

/// `[a, ...b[], c]` with exactly one length-less spread — an array, or an open
/// tuple whose own fixed slots join the outer ones — is an open tuple. A spread
/// of an unresolved operand, or a second length-less one, still has no shape.
fn open_tuple_from_operands(elements: &[(bool, Type)]) -> Option<Type> {
    let mut leading = Vec::new();
    let mut rest: Option<Type> = None;
    let mut trailing = Vec::new();
    for (is_rest, ty) in elements {
        let slots = if rest.is_none() { &mut leading } else { &mut trailing };
        if !*is_rest {
            slots.push(ty.clone());
            continue;
        }
        if let Some(members) = spread_operand_members(ty) {
            slots.extend(members);
            continue;
        }
        if rest.is_some() {
            return None;
        }
        let peeled = match ty {
            Type::Reference(_) => crate::program::with_dts_expansion_reason(
                crate::program::DtsExpansionReason::ConditionalType,
                || ty.peeled(),
            ),
            other => other.clone(),
        };
        match peeled {
            Type::Array(element) => rest = Some(*element),
            Type::OpenTuple(inner) => {
                leading.extend(inner.leading);
                rest = Some(*inner.rest);
                trailing.extend(inner.trailing);
            }
            _ => return None,
        }
    }
    let rest = rest?;
    Some(Type::OpenTuple(surge_ts_types::OpenTupleType {
        leading,
        rest: Box::new(rest),
        trailing,
    }))
}

fn splice_spread_operands(elements: &[(bool, Type)]) -> Option<Vec<Type>> {
    let mut members = Vec::with_capacity(elements.len());

    for (is_rest, ty) in elements {
        if !*is_rest {
            members.push(ty.clone());
            continue;
        }
        members.extend(spread_operand_members(ty)?);
    }

    Some(members)
}

/// The elements a `...T` operand contributes, or `None` when its length is not
/// known. A deferred alias instantiation carries its tuple behind a lazy
/// reference, so the operand is peeled before giving up.
fn spread_operand_members(ty: &Type) -> Option<Vec<Type>> {
    match ty {
        Type::Tuple(members) => Some(members.clone()),
        Type::Reference(_) => match crate::program::with_dts_expansion_reason(
            crate::program::DtsExpansionReason::ConditionalType,
            || ty.peeled(),
        ) {
            Type::Tuple(members) => Some(members),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn resolve_function_type(
    function_type: std::sync::Arc<ParsedFunctionType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let local_substitution = extend_substitution_with_type_parameters(
        substitution,
        &function_type.type_parameters,
        &function_type.parameters,
        ctx,
        resolving,
    );

    // A written constraint or default (`<T extends C = D>() => …`) is a type
    // reference like any other, resolved for its own diagnostics — but only
    // where it is written. Resolving it again beneath another declaration's
    // expansion buys no diagnostic and walks the constraint's own generic
    // graph at every use site, which on zod's `.d.ts` chains does not
    // terminate in reasonable time.
    let written_here = resolving.is_empty();
    for type_parameter in function_type.type_parameters.iter().take_while(|_| written_here) {
        for written in [&type_parameter.constraint, &type_parameter.default_type]
            .into_iter()
            .flatten()
        {
            let _ = super::resolve_parsed_type(written.clone(), ctx, resolving, &local_substitution);
        }
    }

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

    // A parameter's name is in scope for the parameters after it and for the
    // return type: `(x: number) => typeof x`, `({ a: alias }: T) => typeof alias`.
    let outer_parameter_bindings = ctx.signature_parameter_bindings.clone();
    for parameter in function_type.parameters.iter().cloned() {
        let is_this = parameter.is_this;
        let name = parameter.name.clone();
        let bound_names = parameter.bound_names.clone();
        let is_rest = parameter.rest;
        let resolved_parameter =
            resolve_function_type_parameter(parameter, ctx, resolving, &local_substitution);
        had_error |= resolved_parameter.had_error;
        if !is_this {
            let bound_type = if is_rest {
                Type::Array(Box::new(resolved_parameter.ty.clone()))
            } else {
                resolved_parameter.ty.clone()
            };
            if bound_names.is_empty() {
                if let Some(name) = name {
                    ctx.signature_parameter_bindings.push((name, bound_type));
                }
            } else {
                for bound in &bound_names {
                    let ty = crate::checks::function::bound_name_type(&bound_type, bound);
                    ctx.signature_parameter_bindings.push((bound.name.clone(), ty));
                }
            }
        }
        // The `this` parameter is resolved so an unresolved `this` type still
        // reports once (and propagates `had_error` to avoid a cascade), but it is
        // not a real call parameter, so it is excluded from arity and arguments.
        if is_this {
            continue;
        }
        parameters.push(resolved_parameter.ty);
    }

    let return_type = resolve_parsed_type(
        (*function_type.return_type).clone(),
        ctx,
        resolving,
        &local_substitution,
    );
    ctx.signature_parameter_bindings = outer_parameter_bindings;
    had_error |= return_type.had_error;
    let mut resolved_function = alloc_function_type(
        parameters,
        return_type.ty,
        is_variadic,
        required_parameter_count,
    )
    .with_parameter_names(written_parameter_names(&value_parameters))
    .with_type_parameter_head(crate::checks::function::type_parameter_head(
        &function_type.type_parameters,
    ));
    // A generic signature's own type parameters are erased above (`T` maps to
    // the sentinel), so a call through the resolved handle could not infer
    // them — every `find<T>(type: Type<T>): Collection<T>` on an interface
    // returned `unknown`. The written signature rides on the handle, with the
    // enclosing bindings the body was resolved under, so the call site
    // re-instantiates it from its arguments.
    if (!function_type.type_parameters.is_empty()
        || matches!(*function_type.return_type, ParsedType::Predicate(_)))
        && let Some(declared) =
            crate::checks::call::DeclaredMemberSignature::capture(&function_type, substitution, ctx)
    {
        resolved_function = resolved_function.with_declaration(std::sync::Arc::new(declared));
    }
    ResolvedType {
        ty: Type::Function(resolved_function),
        had_error,
    }
}

/// The names as written, so a diagnostic can render `(value: string) => void`
/// the way tsc does. Display-only: they are attached to the type handle and
/// never reach the interned payload or assignability.
pub(crate) fn written_parameter_names(
    parameters: &[surge_ts_syntax::ParsedFunctionTypeParameter],
) -> Vec<Option<std::sync::Arc<str>>> {
    parameters
        .iter()
        .map(|parameter| parameter.name.as_deref().map(std::sync::Arc::<str>::from))
        .collect()
}

/// [`resolve_function_type`] with lazy components (Stage 2 of member-level
/// lazy expansion): the FunctionType shell — arity, variadic, required count,
/// `this`/rest handling — is built exactly as the eager path builds it, but an
/// eligible parameter or return annotation becomes a lazy component reference
/// resolved on first read. `this` and rest parameters always resolve eagerly
/// (`this` is typing metadata outside arity; a rest annotation is consumed by
/// variadic call checking, which a deferred wrapper would mis-shape).
pub(crate) fn resolve_function_type_lazy_components(
    function_type: std::sync::Arc<ParsedFunctionType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
    interface_name: &str,
    declaration_start: usize,
    member_index: usize,
    member_name: &str,
) -> ResolvedType {
    let local_substitution = extend_substitution_with_type_parameters(
        substitution,
        &function_type.type_parameters,
        &function_type.parameters,
        ctx,
        resolving,
    );
    let mut substitution_fingerprint: Option<u64> = None;

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

    let mut value_index = 0usize;
    for parameter in function_type.parameters.iter() {
        let is_this = parameter.is_this;
        let is_rest = parameter.rest;
        let defer = !is_this && !is_rest && defer_method_component_annotation(&parameter.ty);
        if defer {
            let ty = super::super::cache::make_lazy_method_component_reference(
                ctx,
                super::super::cache::LazyMemberIdentity {
                    interface_name,
                    declaration_start,
                    member_index,
                    member_name,
                    component: Some(crate::infer::LazySignatureComponent::Parameter(value_index)),
                    substitution_fingerprint: *substitution_fingerprint.get_or_insert_with(|| {
                        super::super::cache::member_substitution_fingerprint(&local_substitution)
                    }),
                },
                &parameter.ty,
                &local_substitution,
            );
            parameters.push(ty);
            value_index += 1;
            continue;
        }
        let resolved_parameter =
            resolve_function_type_parameter(parameter.clone(), ctx, resolving, &local_substitution);
        had_error |= resolved_parameter.had_error;
        if is_this {
            continue;
        }
        value_index += 1;
        parameters.push(resolved_parameter.ty);
    }

    // The return annotation stays eager: a call's result flows through the
    // whole program — truthiness narrowing, unions, optional chains — and a
    // deferred `X | undefined` return measurably escapes narrowing (25 tRPC
    // false positives). Parameters have a narrow consumer surface (argument
    // assignability and contextual typing, both of which peel).
    let return_type = resolve_parsed_type(
        (*function_type.return_type).clone(),
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

/// The method-component deferral tier: structured shapes minus anything
/// containing `typeof` (resolved against value tables the captured
/// environment drops) and minus predicates (`x is T` shapes the signature).
fn defer_method_component_annotation(annotation: &ParsedType) -> bool {
    match annotation {
        ParsedType::Object(_)
        | ParsedType::Tuple(_)
        | ParsedType::Union(_)
        | ParsedType::Intersection(_)
        | ParsedType::Function(_)
        | ParsedType::KeyOf(_)
        | ParsedType::IndexedAccess(_)
        | ParsedType::Mapped(_)
        | ParsedType::Conditional(_)
        | ParsedType::TemplateLiteral(_) => !crate::modules::annotation_contains_typeof(annotation),
        ParsedType::Array(element) => defer_method_component_annotation(element),
        _ => false,
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

/// Borrows the parsed object rather than consuming it: every `ParsedType`
/// payload is `Arc`-backed, so cloning a member annotation is a refcount bump
/// while unwrapping the shared object literal would deep-copy its whole
/// property list on each of the hundreds of thousands of resolutions.
pub(crate) fn resolve_object_type(
    object_type: &ParsedObjectType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = PropertyMap::default();
    let mut had_error = false;
    // A member of a type literal is a structural crossing like an interface
    // member: a declaration that re-enters itself through one is legal
    // recursion (see `CheckerContext::type_literal_member_frames`).
    let literal_frame = resolving.len();
    ctx.structural_resolution_frames.push(literal_frame);
    ctx.type_literal_member_frames.push(literal_frame);

    for property in &object_type.properties {
        let property_type = resolve_parsed_type(property.ty.clone(), ctx, resolving, substitution);
        had_error |= property_type.had_error;

        // Same-named function members of one type literal are overloads, exactly
        // as they are in an interface body, so they fold into one permissive
        // signature the same way. Without this the last declaration won and every
        // earlier overload was dropped, which reported a call matching an earlier
        // overload's arity against the last one's (ts-pattern's `.with`).
        if let Some(existing) = properties.get(property.name.as_str())
            && let Type::Function(existing_fn) = &existing.ty
            && let Type::Function(incoming) = &property_type.ty
        {
            let merged =
                crate::infer::types::interface::merge_overload_signatures(existing_fn, incoming);
            let optional = existing.optional && property.optional;
            let method = existing.method || property.is_method;
            properties.insert(
                property.name.as_str().into(),
                if optional {
                    ObjectProperty::optional(Type::Function(merged))
                } else {
                    ObjectProperty::required(Type::Function(merged))
                }
                .with_method(method),
            );
            continue;
        }

        let object_property = if property.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        }
        .with_method(property.is_method)
        .with_readonly(property.readonly);

        properties.insert(property.name.as_str().into(), object_property);
    }

    let string_index_type = object_type
        .string_index_type
        .as_deref()
        .and_then(|index_type| {
            let resolved = resolve_parsed_type(index_type.clone(), ctx, resolving, substitution);
            had_error |= resolved.had_error;
            (!resolved.had_error).then_some(resolved.ty)
        });

    let number_index_type = object_type
        .number_index_type
        .as_deref()
        .and_then(|index_type| {
            let resolved = resolve_parsed_type(index_type.clone(), ctx, resolving, substitution);
            had_error |= resolved.had_error;
            (!resolved.had_error).then_some(resolved.ty)
        });

    let mut resolved_object =
        alloc_object_type(properties, string_index_type).with_number_index_type(number_index_type);
    if object_type.non_primitive {
        resolved_object = resolved_object.with_non_primitive_marker();
    }
    if let Some(display_name) = &object_type.display_name {
        resolved_object = resolved_object.with_alias_name(display_name.clone());
    }
    if let Some(call_signature) = object_type.call_signature.as_deref() {
        let resolved = resolve_parsed_type(
            ParsedType::Function(std::sync::Arc::new(call_signature.clone())),
            ctx,
            resolving,
            substitution,
        );
        had_error |= resolved.had_error;
        if let Type::Function(function_type) = resolved.ty {
            resolved_object = resolved_object.with_call_signature(function_type);
        }
    }
    if let Some(construct_signature) = object_type.construct_signature.as_deref() {
        let resolved = resolve_parsed_type(
            ParsedType::Function(std::sync::Arc::new(construct_signature.clone())),
            ctx,
            resolving,
            substitution,
        );
        had_error |= resolved.had_error;
        if let Type::Function(function_type) = resolved.ty {
            resolved_object = resolved_object.with_construct_signature(function_type);
        }
    }
    ctx.type_literal_member_frames.pop();
    ctx.structural_resolution_frames.pop();

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
