//! Type alias resolution and built-in utility types (Partial/Record/Pick/Omit/...).

use super::*;


use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedType, TextSpan};
use surge_ts_types::{ObjectProperty, PropertyMap, Type};

use crate::arena::alloc_object_type;
use crate::context::{CheckerContext, DeclarationResolutionKey, convert_span};
use crate::default_lib::is_generated_default_lib_file_name;
use crate::symbols::TypeAliasInfo;

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
        emit_type_alias_cycle(&alias.name, alias.name_span, ctx);
        // tsc only rejects a type alias that references itself *without* an
        // intervening structural type (`type A = A`, `type A = B; type B = A`).
        // Recursion through an object/array/tuple/function (`type Rec<T> = { rest:
        // Rec<T> }`, and the mutually recursive lib/DOM clusters React's event
        // types pull in) is valid — the self-edge is just left unexpanded. Keep the
        // internal cycle marker either way, but only poison the enclosing type
        // (`had_error: true`) for a genuine structureless cycle; a structural one
        // resolves to a clean `unknown` so it does not collapse a generic
        // instantiation in `bind_type_arguments`.
        return ResolvedType {
            ty: Type::Unknown,
            had_error: !alias_body_supports_recursion(&alias.body.ty),
        };
    }

    resolving.push(declaration_key.clone());
    let Some(local_substitution) = bind_type_arguments(
        &alias.body.type_parameters,
        type_arguments,
        &alias.name,
        reference_span.or(alias.name_span),
        ctx,
        resolving,
        substitution,
        pre_resolved_arguments,
    ) else {
        resolving.pop();
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };

    if alias.file_name == "<built-in>" || is_generated_default_lib_file_name(&alias.file_name) {
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

    let namespace_prefix = alias.name.rsplit_once('.').map(|(prefix, _)| prefix.to_string());
    let is_namespace_member = namespace_prefix.is_some();
    if let Some(prefix) = namespace_prefix {
        ctx.namespace_member_resolution_depth += 1;
        ctx.namespace_member_prefix_stack.push(prefix);
    }
    let effective_scope = alias
        .resolution_scope
        .clone()
        .or_else(|| ctx.module_scope_for_file(&alias.file_name));
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
    if is_namespace_member {
        ctx.namespace_member_resolution_depth -= 1;
        ctx.namespace_member_prefix_stack.pop();
    }
    resolving.pop();

    resolved
}

pub(crate) fn resolve_builtin_utility_alias(
    alias_name: &str,
    substitution: &TypeParameterSubstitution,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> Option<ResolvedType> {
    match alias_name {
        "Partial" => Some(resolve_partial_utility_type(substitution)),
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

    let mut properties = PropertyMap::new();
    for (name, property) in object_type.properties.iter() {
        properties.insert(name.clone(), ObjectProperty::optional(property.ty.clone()));
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
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
                PropertyMap::new(),
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
    let mut properties = PropertyMap::new();

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

    let mut properties = PropertyMap::new();
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

    let mut properties = PropertyMap::new();
    for (key, property) in object_type.properties.iter() {
        if keys.iter().any(|candidate| candidate == key) {
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
