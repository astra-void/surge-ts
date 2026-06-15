//! Interface declaration resolution and instance type construction.

use super::*;

use std::collections::BTreeMap;

use typescript_rust_syntax::{ParsedInterfaceMember, ParsedNamedType, ParsedType};
use typescript_rust_types::{ObjectProperty, Type};

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::{CheckerContext, DeclarationResolutionKey};
use crate::default_lib::{is_generated_default_lib_file_name, is_physical_default_lib_file_name};
use crate::symbols::InterfaceInfo;

pub(crate) fn resolve_interface(
    interface: InterfaceInfo,
    type_arguments: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let declaration_key = declaration_resolution_key(&interface.file_name, &interface.name);
    if resolving.iter().any(|name| name == &declaration_key) {
        emit_type_declaration_cycle(&interface.name, interface.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolving.push(declaration_key.clone());
    let Some(local_substitution) = bind_type_arguments(
        &interface.type_parameters,
        type_arguments,
        &interface.name,
        interface.name_span,
        ctx,
        resolving,
        substitution,
    ) else {
        resolving.pop();
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };

    if is_generated_default_lib_file_name(&interface.file_name) {
        match interface.name.as_str() {
            "Array" | "ReadonlyArray" => {
                let element_type = local_substitution.get("T").cloned().unwrap_or(Type::Any);
                resolving.pop();
                return ResolvedType {
                    ty: Type::Array(Box::new(element_type)),
                    had_error: false,
                };
            }
            "Uint8Array" => {
                resolving.pop();
                return ResolvedType {
                    ty: Type::Array(Box::new(Type::Number)),
                    had_error: false,
                };
            }
            "Map" => {
                resolving.pop();
                return ResolvedType {
                    ty: generated_default_lib_map_instance_type(),
                    had_error: false,
                };
            }
            "Promise" | "PromiseLike" => {
                let ty = local_substitution
                    .get("T")
                    .cloned()
                    .unwrap_or(Type::Unknown);
                resolving.pop();
                return ResolvedType {
                    ty,
                    had_error: false,
                };
            }
            _ => {}
        }
    }

    // Physical default libs: `await` is stripped at parse time, so model
    // `Promise<T>`/`PromiseLike<T>` as their resolved value `T` (an implicit
    // await everywhere). This mirrors the generated-lib behaviour and lets
    // async/await code typecheck against the resolved type. `.then()`-style
    // chaining on a raw promise remains a documented limitation.
    if is_physical_default_lib_file_name(&interface.file_name)
        && matches!(interface.name.as_str(), "Promise" | "PromiseLike")
    {
        let ty = local_substitution
            .get("T")
            .cloned()
            .unwrap_or(Type::Unknown);
        resolving.pop();
        return ResolvedType {
            ty,
            had_error: false,
        };
    }

    let resolved = with_type_declaration_scope(&interface.resolution_scope, ctx, |ctx| {
        with_file_name(ctx, &interface.file_name, |ctx| {
            resolve_interface_declaration(
                &interface.extends,
                &interface.members,
                interface.string_index_type.as_ref(),
                ctx,
                resolving,
                &local_substitution,
            )
        })
    });
    resolving.pop();

    resolved
}

pub(crate) fn resolve_interface_declaration(
    extends: &[ParsedNamedType],
    members: &[ParsedInterfaceMember],
    string_index_type: Option<&ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = BTreeMap::new();
    let mut had_error = false;
    let mut inherited_index_type: Option<Type> = None;

    for base in extends {
        let resolved_base = resolve_named_type(base.clone(), ctx, resolving, substitution);
        if resolved_base.ty == Type::Unknown {
            had_error |= resolved_base.had_error;
            continue;
        }

        had_error |= resolved_base.had_error;

        match resolved_base.ty {
            Type::Object(object_type) => {
                for (name, property) in object_type.properties.iter() {
                    properties.entry(name.clone()).or_insert(property.clone());
                }
                if inherited_index_type.is_none() {
                    if let Some(index_type) = &object_type.string_index_type {
                        inherited_index_type = Some(index_type.as_ref().clone());
                    }
                }
            }
            Type::Any => {}
            _ => {}
        }
    }

    for member in members {
        let property_type = resolve_parsed_type(member.ty.clone(), ctx, resolving, substitution);
        had_error |= property_type.had_error;
        let object_property = if member.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        };

        properties.insert(member.name.clone(), object_property);
    }

    // An own index signature takes precedence; otherwise inherit one from a
    // base interface (e.g. `interface ProcessEnv extends Dict<string>`).
    let resolved_index_type = match string_index_type {
        Some(parsed) => {
            let resolved = resolve_parsed_type(parsed.clone(), ctx, resolving, substitution);
            had_error |= resolved.had_error;
            Some(resolved.ty)
        }
        None => inherited_index_type,
    };

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, resolved_index_type)),
        had_error,
    }
}

pub(crate) fn generated_default_lib_map_instance_type() -> Type {
    let mut properties = BTreeMap::new();
    properties.insert(
        "get".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any],
            Type::Any,
            false,
            1,
        ))),
    );
    properties.insert(
        "set".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any, Type::Any],
            Type::Any,
            false,
            2,
        ))),
    );
    properties.insert(
        "has".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any],
            Type::Boolean,
            false,
            1,
        ))),
    );
    properties.insert(
        "delete".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any],
            Type::Boolean,
            false,
            1,
        ))),
    );
    properties.insert(
        "clear".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![],
            Type::Void,
            false,
            0,
        ))),
    );
    properties.insert("size".to_string(), ObjectProperty::required(Type::Number));

    Type::Object(alloc_object_type(properties, None))
}
