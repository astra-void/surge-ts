//! Interface declaration resolution and instance type construction.

use super::*;


use surge_ts_syntax::{ParsedFunctionType, ParsedInterfaceMember, ParsedNamedType, ParsedType};
use surge_ts_types::{FunctionType, ObjectProperty, PropertyMap, Type};

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::{CheckerContext, DeclarationResolutionKey};
use crate::default_lib::{is_generated_default_lib_file_name, is_physical_default_lib_file_name};
use crate::symbols::InterfaceInfo;

pub(crate) fn resolve_interface(
    interface: &InterfaceInfo,
    type_arguments: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let declaration_key = declaration_resolution_key(&interface.file_name, &interface.name);
    if let Some(index) = resolving.iter().position(|name| name == &declaration_key) {
        // A recursive interface (`interface Node { next: Node }`, and the mutually
        // recursive DOM/typed-array/iterator clusters React's event types pull in)
        // is valid in tsc — only the self-referential edge is left unexpanded. The
        // internal cycle marker is still emitted (cycle detection / cascade
        // guarding), but the back-edge resolves to a clean `unknown` with
        // `had_error: false` so the cycle does not poison the enclosing type: a
        // generic instantiation whose argument transitively recurses must still
        // resolve its other members rather than collapse to `unknown` in
        // `bind_type_arguments`.
        ctx.note_resolution_cycle(index);
        emit_type_declaration_cycle(&interface.name, interface.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    resolving.push(declaration_key.clone());
    let Some(local_substitution) = bind_type_arguments(
        &interface.body.type_parameters,
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

    if is_physical_default_lib_file_name(&interface.file_name) {
        match interface.name.as_str() {
            // The lib declares `Array`/`ReadonlyArray` as interfaces, but they
            // model the same structure as the `T[]` syntax (which lowers to
            // `Type::Array`). Collapse them so an `Array<T>` annotation and a
            // `T[]` annotation are the same type and compare assignable; their
            // members (`map`, `concat`, `length`, …) are served by the array
            // apparent-type path. This mirrors the generated-lib behaviour.
            "Array" | "ReadonlyArray" => {
                let element_type = local_substitution.get("T").cloned().unwrap_or(Type::Any);
                resolving.pop();
                return ResolvedType {
                    ty: Type::Array(Box::new(element_type)),
                    had_error: false,
                };
            }
            // `await` is stripped at parse time, so model `Promise<T>` /
            // `PromiseLike<T>` as their resolved value `T` (an implicit await
            // everywhere). `.then()`-style chaining on a raw promise remains a
            // documented limitation.
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

    let namespace_prefix = interface
        .name
        .rsplit_once('.')
        .map(|(prefix, _)| prefix.to_string());
    let is_namespace_member = namespace_prefix.is_some();
    if let Some(prefix) = namespace_prefix {
        ctx.namespace_member_resolution_depth += 1;
        ctx.namespace_member_prefix_stack.push(prefix);
    }
    let resolved = with_type_declaration_scope(&interface.resolution_scope, ctx, |ctx| {
        with_file_name(ctx, &interface.file_name, |ctx| {
            resolve_interface_declaration(
                &interface.body.extends,
                &interface.body.members,
                interface.body.string_index_type.as_ref(),
                interface.body.call_signature.as_ref(),
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

pub(crate) fn resolve_interface_declaration(
    extends: &[ParsedNamedType],
    members: &[ParsedInterfaceMember],
    string_index_type: Option<&ParsedType>,
    call_signature: Option<&ParsedFunctionType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = PropertyMap::new();
    let mut had_error = false;
    let mut inherited_index_type: Option<Type> = None;
    // A base that resolves to `any` (e.g. a mixin) leaves the derived member set
    // unknown; tsc keeps the type open. A base that fails to resolve is only
    // treated as open inside declaration files, where the real base is assumed to
    // be an unmodelled lib type (DOM/Node `Request`) that tsc resolves under
    // `skipLibCheck`; in user source an unresolved base is a genuine error and
    // tsc still flags missing-member access, so it must stay closed there.
    let in_declaration_file = is_declaration_file_name(&ctx.file_name);
    let mut base_is_open = false;

    for base in extends {
        let resolved_base = resolve_named_type(base.clone(), ctx, resolving, substitution);
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
                // An empty-object base inside a declaration file is, in this
                // checker, an unmodelled lib/dependency stub (e.g. the generated
                // `interface Request {}` placeholder for the DOM type). tsc has the
                // real, populated base under `skipLibCheck`, so keep the derived
                // type open instead of flagging every inherited access.
                if in_declaration_file
                    && object_type.properties.is_empty()
                    && object_type.string_index_type.is_none()
                {
                    base_is_open = true;
                }
            }
            Type::Any => base_is_open = true,
            Type::Unknown => base_is_open |= in_declaration_file,
            _ => {}
        }
    }

    for member in members {
        let property_type = resolve_parsed_type(member.ty.clone(), ctx, resolving, substitution);
        had_error |= property_type.had_error;

        // Same-named function members are overloads (within one interface, or
        // merged across declaration-merged interfaces such as `ArrayConstructor`
        // gaining `from` overloads in lib.es2015.core). Collapse them into one
        // permissive signature so a call matching any overload's arity is
        // accepted, rather than last-wins dropping every overload but one.
        if let (Some(existing), Type::Function(incoming)) =
            (properties.get(&member.name), &property_type.ty)
            && let Type::Function(existing_fn) = &existing.ty
        {
            let merged = merge_overload_signatures(existing_fn, incoming);
            let optional = existing.optional && member.optional;
            properties.insert(
                member.name.clone(),
                if optional {
                    ObjectProperty::optional(Type::Function(merged))
                } else {
                    ObjectProperty::required(Type::Function(merged))
                },
            );
            continue;
        }

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
        None => inherited_index_type.or(if base_is_open { Some(Type::Any) } else { None }),
    };

    let mut object_type = alloc_object_type(properties, resolved_index_type);
    if let Some(call_signature) = call_signature {
        let resolved = resolve_parsed_type(
            ParsedType::Function(call_signature.clone()),
            ctx,
            resolving,
            substitution,
        );
        had_error |= resolved.had_error;
        if let Type::Function(function_type) = resolved.ty {
            object_type = object_type.with_call_signature(function_type);
        }
    }

    ResolvedType {
        ty: Type::Object(object_type),
        had_error,
    }
}

/// Collapse two function overloads into a single permissive signature: the
/// required-parameter count is the smaller of the two (a call matching the
/// shorter overload's arity is accepted), the parameter list is the longer of
/// the two with positions widened to `any` where the overloads disagree (so the
/// merge never rejects an argument valid under either overload), and the result
/// is variadic if either overload is. The shorter overload's return type is kept
/// as the representative, matching the most basic form (e.g. `Array.from`'s
/// `T[]`).
fn merge_overload_signatures(a: &FunctionType, b: &FunctionType) -> FunctionType {
    let (longer, shorter) = if a.parameters().len() >= b.parameters().len() {
        (a, b)
    } else {
        (b, a)
    };

    let parameters = longer
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, ty)| match shorter.parameters().get(index) {
            Some(other) if other == ty => ty.clone(),
            Some(_) => Type::Any,
            None => ty.clone(),
        })
        .collect::<Vec<_>>();

    alloc_function_type(
        parameters,
        shorter.return_type().clone(),
        a.is_variadic() || b.is_variadic(),
        a.required_parameter_count()
            .min(b.required_parameter_count()),
    )
}

fn is_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}

pub(crate) fn generated_default_lib_map_instance_type() -> Type {
    let mut properties = PropertyMap::new();
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
