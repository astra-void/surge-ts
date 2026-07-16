//! Type alias resolution and built-in utility types (Partial/Record/Pick/Omit/...).

use super::*;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedType, TextSpan};
use surge_ts_types::{ObjectProperty, PropertyMap, Type};

use crate::arena::alloc_object_type;
use crate::context::{CheckerContext, DeclarationResolutionKey, convert_span};
use crate::default_lib::{is_generated_default_lib_file_name, is_physical_default_lib_file_name};
use crate::symbols::{TypeAliasInfo, TypeDeclarationHandle};

/// Whether a type alias body introduces a structural boundary that makes a
/// self-reference legal (tsc's rule for recursive type aliases). Object, array,
/// tuple, function, mapped and template-literal bodies all qualify; a union or
/// intersection qualifies when any member does. A bare alias reference,
/// conditional, indexed access, etc. do not, so `type A = A` stays an error.
fn alias_body_supports_recursion(ty: &ParsedType) -> bool {
    match ty {
        ParsedType::Object(_)
        | ParsedType::Array(_)
        | ParsedType::Tuple(_)
        | ParsedType::Function(_)
        | ParsedType::Mapped(_)
        | ParsedType::TemplateLiteral(_) => true,
        ParsedType::Union(members) | ParsedType::Intersection(members) => {
            members.iter().any(alias_body_supports_recursion)
        }
        _ => false,
    }
}

pub(crate) fn resolve_type_alias(
    alias: &TypeAliasInfo,
    handle: TypeDeclarationHandle,
    type_arguments: Vec<ParsedType>,
    reference_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
    pre_resolved_arguments: Option<&[Type]>,
) -> ResolvedType {
    let declaration_key = declaration_resolution_key(&alias.file_name, &alias.name);
    if let Some(index) = resolving.iter().position(|name| name == &declaration_key) {
        ctx.note_resolution_cycle(index);
        // tsc only rejects a type alias that references itself *without* an
        // intervening structural type (`type A = A`, `type A = B; type B = A`).
        // Recursion through an object/array/tuple/function (`type Rec = { rest: Rec
        // }`) is valid and tsc reports nothing. For a *non-generic* structural alias
        // resolve the legal back-edge to a lazy nominal reference to the same
        // declaration: forcing it (a member access, an assignability probe) peels one
        // level back to the real recursive shape rather than `unknown`, so a
        // property/assignability check through the self-edge is not silently dropped.
        // The lazy peel stack bounds the re-expansion.
        //
        // A *generic* recursive declaration is left as `unknown`: its lazy peel is
        // bounded mid-instantiation, so forcing the deeply self-instantiating generic
        // clusters (a fluent builder whose every method returns `Builder<…refined…>`)
        // would expose an incomplete shape and over-report member/assignability
        // checks. Keeping `unknown` there preserves the previous (sound, if
        // under-reporting) behaviour. The (suppressed) note marks the degraded
        // resolution; a genuine structureless cycle keeps it as a real error.
        // A re-entry whose path passed through a structural frame (an interface
        // body, or a structural alias body) is legal recursion even when this
        // alias's own body is a bare union of named types — e.g. zod's
        // `type $ZodIssue = … | $ZodIssueInvalidUnion` whose member interface
        // carries `errors: $ZodIssue[][]`. Only a structureless chain
        // (`type A = B; type B = A`) is a genuine tsc error.
        let structural_crossing = ctx
            .structural_resolution_frames
            .iter()
            .any(|&frame| frame > index);
        let legal_recursion = alias_body_supports_recursion(&alias.body.ty) || structural_crossing;
        if legal_recursion && alias.body.type_parameters.is_empty() {
            return ResolvedType {
                ty: make_recursive_cycle_reference(
                    ctx,
                    &alias.name,
                    handle,
                    declaration_key,
                    type_arguments,
                    pre_resolved_arguments,
                    substitution,
                ),
                had_error: false,
            };
        }
        if !legal_recursion {
            emit_type_alias_cycle(&alias.name, alias.name_span, ctx);
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: !legal_recursion,
        };
    }

    resolving.push(declaration_key.clone());
    let effective_scope = alias
        .resolution_scope
        .clone()
        .or_else(|| ctx.module_scope_for_file(&alias.file_name));
    let Some(bound_arguments) = bind_type_arguments(
        &alias.body.type_parameters,
        type_arguments,
        &alias.name,
        reference_span.or(alias.name_span),
        ctx,
        resolving,
        substitution,
        pre_resolved_arguments,
        Some((&effective_scope, &alias.file_name)),
    ) else {
        resolving.pop();
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };
    let arguments_had_error = bound_arguments.had_error;
    let local_substitution = bound_arguments.substitution;

    let from_default_lib =
        alias.file_name == "<built-in>" || is_generated_default_lib_file_name(&alias.file_name);
    // The physical lib models `Pick`/`Omit` as homomorphic mapped types
    // (`{[P in K]: T[P]}`) that should preserve each source property's optional
    // modifier, but `resolve_mapped_type` forces them required. Resolve them as
    // builtin utilities (which clone the source property, keeping optionality)
    // even from the physical lib.
    let physical_modifier_utility = is_physical_default_lib_file_name(&alias.file_name)
        && matches!(
            alias.name.as_str(),
            "Pick" | "Omit" | "Required" | "Readonly"
        );
    if from_default_lib || physical_modifier_utility {
        if let Some(resolved) = resolve_builtin_utility_alias(
            &alias.name,
            &local_substitution,
            reference_span.or(alias.name_span),
            ctx,
        ) {
            resolving.pop();
            return resolved;
        }
    }

    if alias.file_name == "<built-in>" && (alias.name == "Array" || alias.name == "ReadonlyArray") {
        resolving.pop();
        let element_type = local_substitution.get("T").cloned().unwrap_or(Type::Any);
        return ResolvedType {
            ty: Type::Array(Box::new(element_type)),
            had_error: false,
        };
    }

    // Derive the namespace prefix from the *original* declared name, not the local
    // binding: a namespace member imported by name (`import { MouseEventHandler }
    // from "react"`) is renamed to its bare form, but its body still references
    // siblings (`EventHandler`, `MouseEvent`) that only resolve under the `React.`
    // prefix. `declared_name` preserves the qualified source name (`React.X`).
    let namespace_prefix = alias
        .declared_name
        .as_deref()
        .unwrap_or(&alias.name)
        .rsplit_once('.')
        .map(|(prefix, _)| prefix.to_string());
    let is_namespace_member = namespace_prefix.is_some();
    if let Some(prefix) = namespace_prefix {
        ctx.namespace_member_resolution_depth += 1;
        ctx.namespace_member_prefix_stack.push(prefix);
    }
    // A structural alias body (object/array/function/…) is a structural
    // crossing, like an interface body: a cycle re-entered through it is legal
    // recursion (see `CheckerContext::structural_resolution_frames`).
    let structural_frame = alias_body_supports_recursion(&alias.body.ty);
    if structural_frame {
        ctx.structural_resolution_frames.push(resolving.len() - 1);
    }
    ctx.push_type_parameter_constraints_only(&alias.body.type_parameters);
    let resolved = with_type_declaration_scope(&effective_scope, ctx, |ctx| {
        with_file_name(ctx, &alias.file_name, |ctx| {
            resolve_parsed_type_with_substitution(
                alias.body.ty.clone(),
                ctx,
                resolving,
                &local_substitution,
            )
        })
    });
    ctx.pop_type_parameter_scope();
    if structural_frame {
        ctx.structural_resolution_frames.pop();
    }
    if is_namespace_member {
        ctx.namespace_member_resolution_depth -= 1;
        ctx.namespace_member_prefix_stack.pop();
    }
    resolving.pop();

    ResolvedType {
        ty: resolved.ty,
        had_error: resolved.had_error || arguments_had_error,
    }
}

pub(crate) fn resolve_builtin_utility_alias(
    alias_name: &str,
    substitution: &TypeParameterSubstitution,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> Option<ResolvedType> {
    match alias_name {
        "Partial" => Some(resolve_partial_utility_type(substitution)),
        "Required" => Some(resolve_required_utility_type(substitution)),
        "Readonly" => Some(resolve_readonly_utility_type(substitution)),
        "Record" => Some(resolve_record_utility_type(substitution)),
        "Pick" => Some(resolve_pick_utility_type(substitution, name_span, ctx)),
        "Omit" => Some(resolve_omit_utility_type(substitution)),
        "Parameters" => Some(resolve_parameters_utility_type(substitution)),
        "ReturnType" => Some(resolve_return_type_utility_type(substitution)),
        _ => None,
    }
}

pub(crate) fn resolve_partial_utility_type(
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type.peeled() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let mut properties = PropertyMap::default();
    for (name, property) in object_type.properties.iter() {
        properties.insert(name.clone(), ObjectProperty::optional(property.ty.clone()));
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error: false,
    }
}

/// `Required<T>`: every property of `T` becomes required (the inverse of
/// `Partial`). The lib models it as `{ [P in keyof T]-?: T[P] }`, whose `-?`
/// modifier `parse_mapped_type` cannot represent, so resolve it directly here.
pub(crate) fn resolve_required_utility_type(
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type.peeled() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    // `Required<T>` only strips the optional *modifier* (`-?`); it keeps each
    // property's declared type intact, including an explicit `| undefined` member
    // (`jitter?: boolean | … | undefined` stays assignable from `undefined`).
    let mut properties = PropertyMap::default();
    for (name, property) in object_type.properties.iter() {
        properties.insert(name.clone(), ObjectProperty::required(property.ty.clone()));
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(
            properties,
            object_type.string_index_type.as_deref().cloned(),
        )),
        had_error: false,
    }
}

/// `Readonly<T>`: identity for our purposes — surge does not model the `readonly`
/// modifier, so the type is structurally unchanged. The lib models it as
/// `{ readonly [P in keyof T]: T[P] }`, which `parse_mapped_type` degrades; clone
/// the source object's shape instead.
pub(crate) fn resolve_readonly_utility_type(
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type.peeled() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    ResolvedType {
        ty: Type::Object(object_type),
        had_error: false,
    }
}

pub(crate) fn resolve_record_utility_type(
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let Some(key_type) = substitution.get("K").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    if key_type == Type::String {
        return ResolvedType {
            ty: Type::Object(alloc_object_type(
                PropertyMap::default(),
                Some(substitution.get("T").cloned().unwrap_or(Type::Unknown)),
            )),
            had_error: false,
        };
    }

    let Some(keys) = string_literal_union_keys(&key_type) else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let value_type = substitution.get("T").cloned().unwrap_or(Type::Unknown);
    let mut properties = PropertyMap::default();

    for key in keys {
        properties.insert(key, ObjectProperty::required(value_type.clone()));
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error: false,
    }
}

pub(crate) fn resolve_pick_utility_type(
    substitution: &TypeParameterSubstitution,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type.peeled() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(key_type) = substitution.get("K").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(keys) = string_literal_union_keys(&key_type) else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let mut properties = PropertyMap::default();
    for key in keys {
        let Some(property) = object_type.properties.get(&key) else {
            let key_type_name = key_type.name();
            let constraint_name = format!("keyof {}", Type::Object(object_type.clone()).name());
            let mut diagnostic =
                Diagnostic::ts2344(&key_type_name, &constraint_name, ctx.file_name.clone());
            if let Some(span) = name_span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push_utility_diagnostic_once(diagnostic);
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        };

        properties.insert(key, property.clone());
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error: false,
    }
}

pub(crate) fn resolve_omit_utility_type(substitution: &TypeParameterSubstitution) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type.peeled() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(key_type) = substitution.get("K").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(keys) = string_literal_union_keys(&key_type) else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let keys: std::collections::HashSet<&str> = keys.iter().map(String::as_str).collect();
    let mut properties = PropertyMap::default();
    for (key, property) in object_type.properties.iter() {
        if keys.contains(key.as_str()) {
            continue;
        }

        properties.insert(key.clone(), property.clone());
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error: false,
    }
}

pub(crate) fn resolve_parameters_utility_type(
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Function(function_type) = source_type.peeled() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    ResolvedType {
        ty: Type::Tuple(function_type.parameters().to_vec()),
        had_error: false,
    }
}

pub(crate) fn resolve_return_type_utility_type(
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Function(function_type) = source_type.peeled() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    ResolvedType {
        ty: function_type.return_type().clone(),
        had_error: false,
    }
}

pub(crate) fn string_literal_union_keys(ty: &Type) -> Option<Vec<String>> {
    match ty {
        Type::StringLiteral(value) => Some(vec![value.clone()]),
        Type::Union(union) => {
            let mut keys = Vec::new();
            for variant in union.types() {
                match variant {
                    Type::StringLiteral(value) => keys.push(value.clone()),
                    _ => return None,
                }
            }
            Some(keys)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use surge_ts_types::ObjectType;

    fn optional_object() -> Type {
        let mut props = PropertyMap::default();
        props.insert("a".to_string(), ObjectProperty::optional(Type::Number));
        props.insert("b".to_string(), ObjectProperty::optional(Type::String));
        Type::Object(ObjectType::new(props, None))
    }

    #[test]
    fn omit_preserves_source_property_optionality() {
        let mut sub = TypeParameterSubstitution::new();
        sub.insert("T".to_string(), optional_object());
        sub.insert("K".to_string(), Type::StringLiteral("b".to_string()));

        let resolved = resolve_omit_utility_type(&sub);
        let Type::Object(object) = resolved.ty else {
            panic!("Omit must resolve to an object");
        };
        let a = object.properties.get("a").expect("`a` is kept");
        assert!(
            a.is_optional(),
            "Omit must keep `a` optional, not force it required"
        );
        assert!(object.properties.get("b").is_none(), "`b` is omitted");
    }
}
