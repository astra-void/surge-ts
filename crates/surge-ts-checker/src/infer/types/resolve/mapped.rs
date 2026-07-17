use super::*;

use surge_ts_syntax::ParsedMappedType;
use surge_ts_types::{ObjectProperty, PropertyMap};

use crate::arena::alloc_object_type;

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
                PropertyMap::default(),
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

        let source_optional = homomorphic_source
            .as_ref()
            .and_then(|object| object.get_property(&key))
            .is_some_and(|property| property.is_optional());
        properties.insert(
            key.into(),
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
