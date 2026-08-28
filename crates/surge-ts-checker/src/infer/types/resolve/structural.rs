use super::*;

use surge_ts_syntax::{ParsedFunctionType, ParsedFunctionTypeParameter, ParsedObjectType};
use surge_ts_types::{ObjectProperty, PropertyMap};

use crate::arena::{alloc_function_type, alloc_object_type};

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
    function_type: std::sync::Arc<ParsedFunctionType>,
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

fn rest_element_type(ty: Type) -> Type {
    match ty {
        Type::Array(element) => *element,
        other => other,
    }
}

/// [`resolve_function_type`] with lazy components (Stage 2 of member-level
/// lazy expansion): the FunctionType shell — arity, variadic, required count,
/// `this`/rest handling — is built exactly as the eager path builds it, but an
/// eligible parameter or return annotation becomes a lazy component reference
/// resolved on first read. `this` and rest parameters always resolve eagerly
/// (`this` is typing metadata outside arity; a rest annotation is stored as
/// its ELEMENT type, which a deferred wrapper would mis-shape).
pub(crate) fn resolve_function_type_lazy_components(
    function_type: std::sync::Arc<ParsedFunctionType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
    interface_name: &str,
    declaration_start: usize,
    member_name: &str,
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

    let mut value_index = 0usize;
    for parameter in function_type.parameters.iter().cloned() {
        let is_this = parameter.is_this;
        let is_rest = parameter.rest;
        let defer = !is_this
            && !is_rest
            && defer_method_component_annotation(&parameter.ty);
        if defer {
            let ty = super::super::cache::make_lazy_method_component_reference(
                ctx,
                interface_name,
                declaration_start,
                member_name,
                crate::infer::LazySignatureComponent::Parameter(value_index),
                parameter.ty,
                &local_substitution,
            );
            parameters.push(ty);
            value_index += 1;
            continue;
        }
        let resolved_parameter =
            resolve_function_type_parameter(parameter, ctx, resolving, &local_substitution);
        had_error |= resolved_parameter.had_error;
        if is_this {
            continue;
        }
        value_index += 1;
        if is_rest {
            parameters.push(rest_element_type(resolved_parameter.ty));
        } else {
            parameters.push(resolved_parameter.ty);
        }
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
        | ParsedType::TemplateLiteral(_) => {
            !crate::modules::annotation_contains_typeof(annotation)
        }
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

    for property in &object_type.properties {
        let property_type = resolve_parsed_type(property.ty.clone(), ctx, resolving, substitution);
        had_error |= property_type.had_error;
        let object_property = if property.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        }
        .with_method(property.is_method);

        properties.insert(property.name.as_str().into(), object_property);
    }

    let string_index_type = object_type.string_index_type.as_deref().and_then(|index_type| {
        let resolved = resolve_parsed_type(index_type.clone(), ctx, resolving, substitution);
        had_error |= resolved.had_error;
        (!resolved.had_error).then_some(resolved.ty)
    });

    let mut resolved_object = alloc_object_type(properties, string_index_type);
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
