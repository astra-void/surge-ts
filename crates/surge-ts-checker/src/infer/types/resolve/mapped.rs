use super::*;

use surge_ts_syntax::ParsedMappedType;
use surge_ts_types::{ObjectProperty, PropertyMap};

use crate::metrics::alloc_object_type;

/// Whether a mapped type's key constraint admits arbitrary members, which makes
/// the result an index signature rather than a fixed property set. Mirrors
/// `record_key_is_open`, the built-in-lib path's rule for the same question.
fn mapped_key_is_open(constraint: &Type) -> bool {
    match constraint {
        Type::String | Type::Number | Type::Symbol => true,
        Type::Union(union) => union.types().iter().any(mapped_key_is_open),
        _ => false,
    }
}

/// The property names a literal key constraint enumerates. A numeric key names
/// the member by its text, the same way an object literal's numeric key does.
fn mapped_literal_keys(constraint: &Type) -> Option<Vec<String>> {
    match constraint {
        Type::StringLiteral(value) => Some(vec![value.clone()]),
        Type::NumberLiteral(literal) => Some(vec![literal.value.clone()]),
        Type::Union(union) => {
            let mut keys = Vec::new();
            for variant in union.types() {
                keys.extend(mapped_literal_keys(variant)?);
            }
            Some(keys)
        }
        _ => None,
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
    // `Partial<C1>` with `C1` a type parameter no enclosing declaration binds
    // is an instantiation surge could not complete (the alias argument was
    // lost on the way in); mapping over it would produce an empty object that
    // then reports every member. tsc has the real keys, so the shape is open.
    if let Some(ParsedType::Named(named)) = keyof_operand.as_ref()
        && named.type_arguments.is_empty()
        && let Some(Type::TypeParameter(parameter)) = substitution.get(&named.name)
        && !ctx.type_parameter_in_scope(parameter.name.as_ref())
    {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }
    let resolved_constraint = resolve_parsed_type(*mapped.constraint, ctx, resolving, substitution);

    if resolved_constraint.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    // A non-literal key constraint maps to an index signature: `{ [P in string]: T }`
    // is `{ [k: string]: T }`, and `number`/`symbol` are as open as `string`
    // (`keyof any` is all three). This is how `Record<K, T>` resolves when it
    // routes through its mapped-type body — the physical lib declares it as
    // `{ [P in K]: T }` — rather than the built-in `resolve_record_utility_type`
    // fast path. Without this the mapped type collapsed to `unknown`, which
    // surfaced as a spurious missing-property error on every read.
    if mapped_key_is_open(&resolved_constraint.ty) {
        let mut value_substitution =
            substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        value_substitution.insert(mapped.key_name.clone(), resolved_constraint.ty.clone());
        let resolved_value =
            resolve_parsed_type(*mapped.value_type, ctx, resolving, &value_substitution);
        return ResolvedType {
            ty: Type::Object(alloc_object_type(
                PropertyMap::default(),
                Some(resolved_value.ty),
            )),
            had_error: resolved_value.had_error,
        };
    }

    let Some(keys) = mapped_literal_keys(&resolved_constraint.ty) else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let homomorphic_source = keyof_operand.and_then(|operand| {
        let resolved = resolve_parsed_type(operand, ctx, resolving, substitution);
        match crate::program::with_dts_expansion_reason(
            crate::program::DtsExpansionReason::MappedType,
            || resolved.ty.peeled(),
        ) {
            Type::Object(object) => Some(object),
            _ => None,
        }
    });

    let _expansion_scope = TypeExpansionScope::enter();
    let mut properties = PropertyMap::default();
    let mut had_error = false;

    for key in keys {
        if !try_consume_type_expansion_step() {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: false,
            };
        }
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

        let source_property = homomorphic_source
            .as_ref()
            .and_then(|object| object.get_property(&key));
        let source_optional = source_property.is_some_and(|property| property.is_optional());
        let source_method = source_property.is_some_and(|property| property.is_method());
        properties.insert(
            key.into(),
            ObjectProperty {
                ty: resolved_value.ty,
                optional: mapped.optional || source_optional,
                method: source_method,
            },
        );
    }

    // Reusing the source's index value type is exact for identity mappings
    // (`T[k]`) and an approximation for transforming ones; either way it keeps
    // index-signature reads legal, matching tsc's homomorphic behaviour.
    let index_type = homomorphic_source
        .as_ref()
        .and_then(|object| object.string_index_type.as_deref().cloned());
    // A source the checker had to leave open (a spread of a value it could
    // not model) stays open through the mapping: `Partial<{ x, ...degraded }>`
    // still admits the members tsc sees through the spread.
    let source_is_open = homomorphic_source
        .as_ref()
        .is_some_and(|object| object.synthetic_open_index);
    let mut mapped_object = alloc_object_type(properties, index_type);
    if source_is_open {
        mapped_object = mapped_object.with_open_index_marker();
    }

    ResolvedType {
        ty: Type::Object(mapped_object),
        had_error,
    }
}
