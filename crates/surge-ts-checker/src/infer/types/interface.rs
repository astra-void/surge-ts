//! Interface declaration resolution and instance type construction.

use super::*;

use surge_ts_syntax::{ParsedFunctionType, ParsedInterfaceMember, ParsedNamedType, ParsedType};
use surge_ts_types::{FunctionType, ObjectProperty, PropertyMap, Type, current_program_type_store};

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::{CheckerContext, DeclarationResolutionKey};
use crate::default_lib::{is_generated_default_lib_file_name, is_physical_default_lib_file_name};
use crate::symbols::{InterfaceInfo, TypeDeclarationHandle};

pub(crate) fn resolve_interface(
    interface: &InterfaceInfo,
    handle: TypeDeclarationHandle,
    type_arguments: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
    pre_resolved_arguments: Option<&[Type]>,
) -> ResolvedType {
    crate::program::record_interface_resolution_attempt();
    let declaration_key = declaration_resolution_key(&interface.file_name, &interface.name);
    if let Some(index) = resolving.iter().position(|name| name == &declaration_key) {
        // A recursive interface (`interface Node { next: Node }`) is always valid in
        // tsc. For a *non-generic* interface resolve the self-edge to a lazy nominal
        // reference to the same declaration so a member/assignability check through it
        // peels back to the real shape instead of silently passing on `unknown`; the
        // lazy peel stack bounds re-expansion.
        //
        // A *generic* interface is left as `unknown` with a (suppressed) note: its
        // lazy peel is bounded mid-instantiation, so forcing the deeply
        // self-instantiating generic builder/library clusters would expose an
        // incomplete shape and over-report. Keeping `unknown` preserves the previous
        // sound-but-under-reporting behaviour for those.
        ctx.note_resolution_cycle(index);
        if interface.body.type_parameters.is_empty() {
            return ResolvedType {
                ty: make_recursive_cycle_reference(
                    ctx,
                    &interface.name,
                    handle,
                    declaration_key,
                    type_arguments,
                    pre_resolved_arguments,
                    substitution,
                ),
                had_error: false,
            };
        }
        emit_type_declaration_cycle(&interface.name, interface.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    }

    resolving.push(declaration_key.clone());
    let declaration_effective_scope = interface
        .resolution_scope
        .clone()
        .or_else(|| ctx.module_scope_for_file(&interface.file_name));
    let Some(bound_arguments) = bind_type_arguments(
        &interface.body.type_parameters,
        type_arguments,
        &interface.name,
        interface.name_span,
        ctx,
        resolving,
        substitution,
        pre_resolved_arguments,
        Some((&declaration_effective_scope, &interface.file_name)),
    ) else {
        resolving.pop();
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };
    let arguments_had_error = bound_arguments.had_error;
    let local_substitution = bound_arguments.substitution;

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

    // Under `noLib`, a configured `types` package may replace the standard lib
    // (roblox-ts's `@rbxts/compiler-types` redeclares `Array`/`ReadonlyArray`
    // with the same structural shape as `T[]`). Collapse them exactly like the
    // physical lib so `Array<T>` unifies with `T[]` and the self-referential
    // generic interface never re-enters its own resolution (which would
    // otherwise degrade to `unknown` via the generic cycle path above).
    //
    // `Promise` is intentionally NOT collapsed here: roblox-ts code uses the
    // Promise object surface (`.then`/`.catch`) directly, so mapping it to its
    // awaited value (as the physical-lib path does for implicit-await) would
    // strip those members and over-report.
    if ctx.options.no_lib
        && matches!(interface.name.as_str(), "Array" | "ReadonlyArray")
        && crate::program::is_configured_types_global_file(&interface.file_name, &ctx.options.types)
    {
        let element_type = local_substitution.get("T").cloned().unwrap_or(Type::Any);
        resolving.pop();
        return ResolvedType {
            ty: Type::Array(Box::new(element_type)),
            had_error: false,
        };
    }

    let physical_default_lib = is_physical_default_lib_file_name(&interface.file_name);
    let collect_all_interface_identities = crate::program::dts_expansion_trace_enabled();
    let stable_declaration = if physical_default_lib || collect_all_interface_identities {
        stable_interface_declaration_id(interface).ok()
    } else {
        None
    };
    let interface_key = if physical_default_lib || collect_all_interface_identities {
        match canonical_physical_interface_key(interface, &local_substitution, ctx) {
            Ok(key) => Some(key),
            Err(reason) => {
                record_interface_cache_skip(reason);
                None
            }
        }
    } else {
        None
    };
    let declaration_template = if physical_default_lib
        && physical_interface_member_cache_enabled()
        && let Some(declaration) = stable_declaration.as_ref()
    {
        physical_interface_declaration_template(ctx, interface, declaration)
    } else {
        None
    };
    let creation_before = crate::program::type_creation_snapshot();
    if physical_default_lib {
        if !physical_interface_cache_enabled() {
            record_interface_cache_skip(InterfaceCacheSkipReason::Disabled);
        } else if let Some(key) = interface_key.as_ref() {
            if let Some(cached) = lookup_physical_interface_instantiation(ctx, key) {
                crate::program::record_program_counter(|c| {
                    c.physical_interface_cache_hit_count += 1
                });
                resolving.pop();
                crate::program::record_interface_resolution_result(
                    stable_declaration,
                    Some(key),
                    true,
                    true,
                    interface.body.extends.len(),
                    interface.body.members.len(),
                    creation_before,
                );
                return ResolvedType {
                    ty: (*cached).clone(),
                    had_error: false,
                };
            }
            crate::program::record_program_counter(|c| c.physical_interface_cache_miss_count += 1);
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
    // An interface body is a structural crossing: a type-alias cycle re-entered
    // through this frame is legal recursion (see
    // `CheckerContext::structural_resolution_frames`).
    ctx.structural_resolution_frames.push(resolving.len() - 1);
    ctx.push_type_parameter_constraints_only(&interface.body.type_parameters);
    let diagnostics_before = ctx.diagnostics().len();
    let utility_keys_before = ctx.utility_diagnostic_keys.len();
    let degradation_before = crate::program::expansion_degradation_epoch();
    let expansion_reason = if physical_default_lib {
        crate::program::DtsExpansionReason::DefaultLibInterfaceInstantiation
    } else if interface.file_name.contains("node_modules") {
        crate::program::DtsExpansionReason::DependencyInterfaceInstantiation
    } else {
        crate::program::DtsExpansionReason::InterfaceResolution
    };
    let resolved = crate::program::with_dts_expansion_reason(expansion_reason, || {
        with_type_declaration_scope(&declaration_effective_scope, ctx, |ctx| {
            with_file_name(ctx, &interface.file_name, |ctx| {
                resolve_interface_declaration(
                    &interface.body.extends,
                    &interface.body.members,
                    interface.body.string_index_type.as_ref(),
                    interface.body.call_signature.as_ref(),
                    &interface.body.construct_signatures,
                    ctx,
                    resolving,
                    &local_substitution,
                    stable_declaration.as_ref(),
                    declaration_template.as_deref(),
                    interface_key.as_ref(),
                )
            })
        })
    });
    ctx.pop_type_parameter_scope();
    ctx.structural_resolution_frames.pop();
    if is_namespace_member {
        ctx.namespace_member_resolution_depth -= 1;
        ctx.namespace_member_prefix_stack.pop();
    }
    resolving.pop();

    let had_error = resolved.had_error || arguments_had_error;
    let emitted_diagnostics = ctx.diagnostics().len() != diagnostics_before
        || ctx.utility_diagnostic_keys.len() != utility_keys_before;
    let degraded_during_expansion =
        crate::program::expansion_degradation_epoch() != degradation_before;
    let clean = !had_error && !emitted_diagnostics && !degraded_during_expansion;
    let mut ty = resolved.ty;
    if physical_default_lib
        && physical_interface_cache_enabled()
        && let Some(key) = interface_key.clone()
    {
        let value_rejection = clean
            .then(|| validate_physical_interface_cache_value(&ty).err())
            .flatten();
        if clean && value_rejection.is_none() {
            ty = (*intern_physical_interface_instantiation(ctx, key, ty)).clone();
        } else {
            crate::program::record_program_counter(|c| {
                c.physical_interface_cache_reject_had_error_count += u64::from(had_error);
                c.physical_interface_cache_reject_diagnostics_count +=
                    u64::from(emitted_diagnostics);
                c.physical_interface_cache_reject_degradation_count +=
                    u64::from(degraded_during_expansion);
                c.physical_interface_cache_reject_unknown_count += u64::from(matches!(
                    value_rejection,
                    Some(InterfaceCacheValueRejection::Unknown)
                ));
                c.physical_interface_cache_reject_context_count += u64::from(matches!(
                    value_rejection,
                    Some(InterfaceCacheValueRejection::ResolutionContext)
                ));
                c.physical_interface_cache_reject_traversal_count += u64::from(matches!(
                    value_rejection,
                    Some(InterfaceCacheValueRejection::TraversalLimit)
                ));
            });
        }
    }
    crate::program::record_interface_resolution_result(
        stable_declaration,
        interface_key.as_ref(),
        clean,
        false,
        interface.body.extends.len(),
        interface.body.members.len(),
        creation_before,
    );

    ResolvedType { ty, had_error }
}

pub(crate) fn resolve_interface_declaration(
    extends: &[ParsedNamedType],
    members: &[ParsedInterfaceMember],
    string_index_type: Option<&ParsedType>,
    call_signature: Option<&ParsedFunctionType>,
    construct_signatures: &[ParsedFunctionType],
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
    interface_declaration: Option<&crate::context::StableInterfaceDeclarationId>,
    declaration_template: Option<&crate::context::InterfaceDeclarationTemplate>,
    interface_key: Option<&crate::context::InterfaceInstantiationKey>,
) -> ResolvedType {
    crate::program::record_interface_member_declaration_visits(members.len());
    crate::program::record_program_counter(|c| {
        c.interface_own_property_map_alloc_count += 1;
        c.interface_method_signature_group_alloc_count += members
            .iter()
            .filter(|member| matches!(member.ty, ParsedType::Function(_)))
            .count() as u64;
        c.interface_call_signature_array_alloc_count += u64::from(call_signature.is_some());
        c.interface_construct_signature_array_alloc_count += construct_signatures.len() as u64;
        c.interface_index_signature_alloc_count += u64::from(string_index_type.is_some());
    });
    let mut properties = PropertyMap::default();
    let mut had_error = false;
    let mut inherited_index_type: Option<Type> = None;
    let mut inherited_call_signature: Option<FunctionType> = None;
    let mut inherited_construct_signature: Option<FunctionType> = None;
    // A base that resolves to `any` (e.g. a mixin) leaves the derived member set
    // unknown; tsc keeps the type open. A base that fails to resolve is only
    // treated as open inside declaration files, where the real base is assumed to
    // be an unmodelled lib type (DOM/Node `Request`) that tsc resolves under
    // `skipLibCheck`; in user source an unresolved base is a genuine error and
    // tsc still flags missing-member access, so it must stay closed there.
    let in_declaration_file = is_declaration_file_name(&ctx.file_name);
    let mut base_is_open = false;
    for base in extends {
        let resolved_base = crate::program::with_dts_expansion_reason(
            crate::program::DtsExpansionReason::InterfaceHeritageResolution,
            || {
                resolve_named_type(
                    std::sync::Arc::new(base.clone()),
                    ctx,
                    resolving,
                    substitution,
                )
            },
        );
        had_error |= resolved_base.had_error;
        // A base that resolved with errors may be missing members surge could
        // not model; the derived member set is incomplete, so keep it open
        // rather than flagging every inherited access.
        base_is_open |= resolved_base.had_error;

        // A generic base (`extends Dict<string>`) resolves to a nominal
        // `Type::Reference`; peel it so its inherited members and index signature
        // are merged structurally.
        match resolved_base.ty.peeled() {
            Type::Object(object_type) => {
                for (name, property) in object_type.properties.iter() {
                    let reason = if matches!(property.ty, Type::Function(_)) {
                        crate::program::DtsExpansionReason::InheritedMethodMerge
                    } else {
                        crate::program::DtsExpansionReason::InheritedPropertyMerge
                    };
                    crate::program::with_dts_expansion_reason(reason, || {
                        properties.entry(name.clone()).or_insert(property.clone());
                    });
                }
                if let Some(declaration) = interface_declaration {
                    let inherited_methods = object_type
                        .properties
                        .values()
                        .filter(|property| matches!(property.ty, Type::Function(_)))
                        .count();
                    crate::program::record_inherited_member_merge(
                        declaration,
                        &base.name,
                        object_type.properties.len(),
                        inherited_methods,
                    );
                }
                if inherited_index_type.is_none() {
                    if let Some(index_type) = &object_type.string_index_type {
                        inherited_index_type = Some(index_type.as_ref().clone());
                    }
                }
                // Call/construct signatures are inherited like members: React's
                // `ForwardRefExoticComponent extends ExoticComponent` carries its
                // callability entirely from the base, and dropping it here strips
                // the component's call signature (breaking `ComponentProps<typeof
                // forwardRefComponent>` and JSX prop checking).
                if inherited_call_signature.is_none() {
                    inherited_call_signature = object_type.call_signature().cloned();
                }
                if inherited_construct_signature.is_none() {
                    inherited_construct_signature = object_type.construct_signature().cloned();
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
            // A degraded (sentinel-`Unknown`) base is surge's own resolution
            // failure — e.g. a base inside a cyclic module cluster — not a user
            // error; tsc has the full base, so the derived type must stay open
            // in user source too or every inherited member access is a false
            // TS2339. Only the *genuine* `unknown` keyword keeps the derived
            // type closed outside declaration files.
            Type::Unknown => base_is_open = true,
            Type::GenuineUnknown => base_is_open |= in_declaration_file,
            _ => {}
        }
    }

    let mut own_method_group_contaminated = std::collections::HashMap::<String, bool>::new();
    let mut own_method_group_clean = std::collections::HashMap::<String, bool>::new();
    for (member_index, member) in members.iter().enumerate() {
        let is_method = matches!(member.ty, ParsedType::Function(_));
        let reason = if is_method {
            crate::program::DtsExpansionReason::InterfaceMethodMapping
        } else {
            crate::program::DtsExpansionReason::InterfaceOwnPropertyMapping
        };
        let member_template = declaration_template.and_then(|template| {
            let member_template = template.members.get(member_index)?;
            (member_template.declaration.declared_name.as_ref() == member.name
                && member_template.declaration.declaration_kind
                    == if is_method {
                        crate::context::InterfaceMemberDeclarationKind::Method
                    } else {
                        crate::context::InterfaceMemberDeclarationKind::Property
                    })
            .then_some(member_template)
        });
        let method_key = is_method
            .then(|| {
                Some(interface_member_instantiation_key(
                    &member_template?.declaration,
                    interface_key?,
                ))
            })
            .flatten();
        let diagnostics_before = ctx.diagnostics().len();
        let utility_keys_before = ctx.utility_diagnostic_keys.len();
        let degradation_before = crate::program::expansion_degradation_epoch();
        let cached_method = method_key
            .as_ref()
            .and_then(|key| lookup_physical_interface_method(ctx, key));
        let method_cache_hit = cached_method.is_some();
        if is_method && method_key.is_some() {
            crate::program::record_program_counter(|c| {
                if method_cache_hit {
                    c.interface_method_cache_hit_count += 1;
                    c.interface_method_function_payload_avoided_count += 1;
                } else {
                    c.interface_method_cache_miss_count += 1;
                }
            });
        }
        let mut property_type = if let Some(function) = cached_method {
            ResolvedType {
                ty: Type::Function(function),
                had_error: false,
            }
        } else {
            crate::program::with_dts_expansion_reason(reason, || {
                resolve_parsed_type(member.ty.clone(), ctx, resolving, substitution)
            })
        };
        let emitted_diagnostics = ctx.diagnostics().len() != diagnostics_before
            || ctx.utility_diagnostic_keys.len() != utility_keys_before;
        let degraded = crate::program::expansion_degradation_epoch() != degradation_before;
        let value_rejection =
            if is_method && !property_type.had_error && !emitted_diagnostics && !degraded {
                validate_physical_interface_cache_value(&property_type.ty).err()
            } else {
                None
            };
        let contextual_typing_dependency = is_method
            && physical_interface_method_has_contextual_typing_dependency(&property_type.ty);
        let method_clean = is_method
            && !property_type.had_error
            && !emitted_diagnostics
            && !degraded
            && !contextual_typing_dependency
            && value_rejection.is_none();
        if is_method {
            let degradation_reason = if method_clean {
                None
            } else if emitted_diagnostics {
                Some(crate::program::InterfaceDegradationReason::DiagnosticProduced)
            } else if matches!(value_rejection, Some(InterfaceCacheValueRejection::Unknown))
                || matches!(property_type.ty, Type::Unknown)
                || degraded
            {
                Some(crate::program::InterfaceDegradationReason::UnknownFallback)
            } else if matches!(
                value_rejection,
                Some(InterfaceCacheValueRejection::ResolutionContext)
            ) {
                Some(crate::program::InterfaceDegradationReason::ContextRetainingReference)
            } else if matches!(
                value_rejection,
                Some(InterfaceCacheValueRejection::TraversalLimit)
            ) {
                Some(crate::program::InterfaceDegradationReason::TraversalLimit)
            } else if contextual_typing_dependency {
                Some(crate::program::InterfaceDegradationReason::ContextualTypingDependency)
            } else if property_type.had_error {
                Some(crate::program::InterfaceDegradationReason::MethodSignatureFailure)
            } else {
                Some(crate::program::InterfaceDegradationReason::Other)
            };
            crate::program::record_interface_method_mapping(
                method_key.as_ref(),
                method_clean,
                degradation_reason,
            );
            if method_key.is_some() {
                crate::program::record_program_counter(|c| {
                    c.interface_method_cache_reject_had_error_count +=
                        u64::from(property_type.had_error);
                    c.interface_method_cache_reject_diagnostics_count +=
                        u64::from(emitted_diagnostics);
                    c.interface_method_cache_reject_degradation_count += u64::from(degraded);
                    c.interface_method_cache_reject_unknown_count += u64::from(matches!(
                        value_rejection,
                        Some(InterfaceCacheValueRejection::Unknown)
                    ));
                    c.interface_method_cache_reject_context_count += u64::from(matches!(
                        value_rejection,
                        Some(InterfaceCacheValueRejection::ResolutionContext)
                    ));
                    c.interface_method_cache_reject_contextual_typing_count +=
                        u64::from(contextual_typing_dependency);
                    c.interface_method_cache_reject_traversal_count += u64::from(matches!(
                        value_rejection,
                        Some(InterfaceCacheValueRejection::TraversalLimit)
                    ));
                });
            }
            if !method_cache_hit
                && method_clean
                && let (Some(key), Type::Function(function)) =
                    (method_key.clone(), &property_type.ty)
            {
                property_type.ty =
                    Type::Function(intern_physical_interface_method(ctx, key, function.clone()));
            }
        }
        had_error |= property_type.had_error;

        if is_method {
            let inherited_function = !own_method_group_contaminated.contains_key(&member.name)
                && properties
                    .get(&member.name)
                    .is_some_and(|property| matches!(property.ty, Type::Function(_)));
            own_method_group_contaminated
                .entry(member.name.clone())
                .or_insert(inherited_function);
            own_method_group_clean
                .entry(member.name.clone())
                .and_modify(|clean| *clean &= method_clean)
                .or_insert(method_clean);
        }

        // Same-named function members are overloads (within one interface, or
        // merged across declaration-merged interfaces such as `ArrayConstructor`
        // gaining `from` overloads in lib.es2015.core). Collapse them into one
        // permissive signature so a call matching any overload's arity is
        // accepted, rather than last-wins dropping every overload but one.
        if let (Some(existing), Type::Function(incoming)) =
            (properties.get(&member.name), &property_type.ty)
            && let Type::Function(existing_fn) = &existing.ty
        {
            let overload_key = member_template
                .filter(|member| member.overload_position != 0)
                .and_then(|member| {
                    let clean = own_method_group_clean
                        .get(member.declaration.declared_name.as_ref())
                        .copied()
                        .unwrap_or(false);
                    let contaminated = own_method_group_contaminated
                        .get(member.declaration.declared_name.as_ref())
                        .copied()
                        .unwrap_or(true);
                    let group = declaration_template?
                        .method_groups
                        .get(member.overload_group? as usize)?;
                    if !clean || contaminated {
                        return None;
                    }
                    Some(interface_overload_instantiation_key(
                        interface_declaration?,
                        group,
                        member.overload_position + 1,
                        interface_key?,
                    ))
                });
            let cached_overload = overload_key
                .as_ref()
                .and_then(|key| lookup_physical_interface_overload(ctx, key));
            let overload_cache_hit = cached_overload.is_some();
            if overload_key.is_some() {
                crate::program::record_program_counter(|c| {
                    if overload_cache_hit {
                        c.interface_overload_cache_hit_count += 1;
                        c.interface_overload_function_payload_avoided_count += 1;
                    } else {
                        c.interface_overload_cache_miss_count += 1;
                    }
                });
            }
            crate::program::record_interface_overload_construction(
                overload_key.as_ref(),
                overload_cache_hit,
            );
            let merged = if let Some(cached) = cached_overload {
                cached
            } else {
                let merged = crate::program::with_dts_expansion_reason(
                    crate::program::DtsExpansionReason::OverloadArrayMerge,
                    || merge_overload_signatures(existing_fn, incoming),
                );
                match overload_key {
                    Some(key)
                        if validate_physical_interface_cache_value(&Type::Function(
                            merged.clone(),
                        ))
                        .is_ok() =>
                    {
                        intern_physical_interface_overload(ctx, key, merged)
                    }
                    Some(_) => {
                        crate::program::record_program_counter(|c| {
                            c.interface_overload_cache_reject_count += 1
                        });
                        merged
                    }
                    None => merged,
                }
            };
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
            let resolved = crate::program::with_dts_expansion_reason(
                crate::program::DtsExpansionReason::InterfaceIndexSignatureMapping,
                || resolve_parsed_type(parsed.clone(), ctx, resolving, substitution),
            );
            had_error |= resolved.had_error;
            Some(resolved.ty)
        }
        None => inherited_index_type.or(if base_is_open { Some(Type::Any) } else { None }),
    };

    let mut object_type = alloc_object_type(properties, resolved_index_type);
    if let Some(call_signature) = call_signature {
        let resolved = crate::program::with_dts_expansion_reason(
            crate::program::DtsExpansionReason::InterfaceCallSignatureMapping,
            || {
                resolve_parsed_type(
                    ParsedType::Function(std::sync::Arc::new(call_signature.clone())),
                    ctx,
                    resolving,
                    substitution,
                )
            },
        );
        had_error |= resolved.had_error;
        if let Type::Function(function_type) = resolved.ty {
            object_type = object_type.with_call_signature(function_type);
        }
    } else if let Some(inherited) = inherited_call_signature {
        object_type = object_type.with_call_signature(inherited);
    }

    // Resolve every construct-signature overload and fold them into one permissive
    // signature (matching how method overloads are merged), so a call matching any
    // overload's arity/arguments is accepted (`new Uint8Array(8)` and
    // `new Uint8Array([1,2,3])` both work).
    let mut merged_construct: Option<FunctionType> = None;
    for construct_signature in construct_signatures {
        let resolved = crate::program::with_dts_expansion_reason(
            crate::program::DtsExpansionReason::InterfaceConstructSignatureMapping,
            || {
                resolve_parsed_type(
                    ParsedType::Function(std::sync::Arc::new(construct_signature.clone())),
                    ctx,
                    resolving,
                    substitution,
                )
            },
        );
        had_error |= resolved.had_error;
        if let Type::Function(function_type) = resolved.ty {
            merged_construct = Some(match merged_construct {
                Some(existing) => crate::program::with_dts_expansion_reason(
                    crate::program::DtsExpansionReason::OverloadArrayMerge,
                    || merge_overload_signatures(&existing, &function_type),
                ),
                None => function_type,
            });
        }
    }
    if let Some(construct_signature) = merged_construct {
        object_type = object_type.with_construct_signature(construct_signature);
    } else if let Some(inherited) = inherited_construct_signature {
        object_type = object_type.with_construct_signature(inherited);
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
    let canonical_merge = a
        .id()
        .zip(b.id())
        .zip(current_program_type_store())
        .map(|((left, right), store)| (left, right, store));
    if let Some((left, right, store)) = canonical_merge.as_ref()
        && let Some(merged) = store.lookup_overload_merge(*left, *right)
    {
        return merged;
    }
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

    let is_variadic = a.is_variadic() || b.is_variadic();
    let required_parameter_count = a
        .required_parameter_count()
        .min(b.required_parameter_count());
    let merged = [a, b]
        .into_iter()
        .find(|candidate| {
            candidate.parameters() == parameters.as_slice()
                && candidate.return_type() == shorter.return_type()
                && candidate.is_variadic() == is_variadic
                && candidate.required_parameter_count() == required_parameter_count
        })
        .cloned()
        .unwrap_or_else(|| {
            crate::program::record_program_counter(|c| c.overload_array_alloc_count += 1);
            alloc_function_type(
                parameters,
                shorter.return_type().clone(),
                is_variadic,
                required_parameter_count,
            )
        });
    match canonical_merge {
        Some((left, right, store)) => store.record_overload_merge(left, right, merged),
        None => merged,
    }
}

fn record_interface_cache_skip(reason: InterfaceCacheSkipReason) {
    crate::program::record_program_counter(|c| match reason {
        InterfaceCacheSkipReason::Disabled => c.physical_interface_cache_skip_disabled_count += 1,
        InterfaceCacheSkipReason::UnstableDeclaration => {
            c.physical_interface_cache_skip_unstable_declaration_count += 1
        }
        InterfaceCacheSkipReason::UnresolvedTypeArgument => {
            c.physical_interface_cache_skip_unresolved_argument_count += 1
        }
        InterfaceCacheSkipReason::UnsupportedTypeArgument => {
            c.physical_interface_cache_skip_unsupported_argument_count += 1
        }
    });
}

fn is_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}

pub(crate) fn generated_default_lib_map_instance_type() -> Type {
    let mut properties = PropertyMap::default();
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
