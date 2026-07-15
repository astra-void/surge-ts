use super::*;

use surge_ts_syntax::ParsedNamedType;

use crate::symbols::TypeDeclarationInfo;

/// Concrete aliases declared by dependencies stay declaration-backed until a
/// semantic consumer needs their structure. This is the critical half of the
/// dependency-surface invariant: eagerly resolving
/// `ComponentPropsWithoutRef<...>` while indexing another declaration's export
/// surface pulls the React/DOM graph into every importing module.
///
/// The escape hatch exists only for before/after profiling and regression
/// isolation; production behavior is lazy.
fn defer_concrete_library_aliases() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_EAGER_DEPENDENCY_ALIASES").as_deref() != Ok("1"))
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
    crate::program::record_generic_instantiation(&decl_key);
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

    // An instantiation is interned/short-circuited only when it is *concrete* —
    // no type parameter is bound in any active scope, so a program-wide entry keyed
    // on (declaration, resolved arguments) matches a standalone resolution. A
    // non-generic function body still pushes an (empty) scope via
    // `with_type_parameter_scope`, so an `is_empty()` check per scope (not stack
    // depth) is the right proxy; otherwise a library generic (`new Uint8Array`)
    // built inside a plain body would be treated as non-concrete and eagerly expand
    // its self-referential cluster into a degraded object instead of a nominal lazy
    // reference. A binding scope active anywhere leaves the resolution
    // context-dependent — an argument can name an in-scope parameter, or the body
    // can capture one and (as measured on zod/ofetch) that capture survives in a
    // form that no cheap post-resolution walk reliably detects — so it is not
    // interned.
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
        && let Some(arguments) = reference_arguments.as_ref()
        && let Some(entry) = lookup_instantiation(ctx, &decl_key, arguments)
    {
        // An interned object with a display form keeps today's nominal wrapping;
        // any other entry (union, reference, primitive, or a display-less bare
        // reference — see the structural interning arm in
        // `tag_generic_object_reference`) is returned structurally, exactly as a
        // fresh expansion would have been, so reuse changes no downstream shape.
        if let Some(display) = alias_display_name.as_deref()
            && matches!(entry.resolved.as_ref(), Type::Object(_))
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
        return ResolvedType {
            ty: (*entry.resolved).clone(),
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

    // Defer a generic *alias* instantiation whose type arguments are all fully
    // resolved (`Omit<ConcreteType, "k">`) even when it is built inside a generic
    // body — `concrete_instantiation` is false whenever *any* type-parameter scope
    // is open, so a utility alias with concrete arguments (`Omit`, `Identity`,
    // `Partial`, `Flatten`; each expanded ~185k× on zod) is otherwise eagerly
    // re-expanded from every reference. Deferring it to a lazy reference that
    // captures this site's substitution expands the body only when a consumer peels
    // it. This is gated on every argument being fully resolved: an argument that
    // collapsed to `unknown` is a placeholder (`Normalize<T>` as a function's
    // return type), and freezing a placeholder-dependent expansion into a shared
    // reference would drop the members a later `T`-substitution should have added.
    //
    // Library-scoped aliases defer even at a concrete site: expanding
    // `ComponentPropsWithoutRef<…>` eagerly opens its type-parameter scope, which
    // turns every nested generic interface reference non-concrete and therefore
    // eager, recursing through the React/DOM interface web (unnamed: 18GB in 45s
    // while collecting dependency export tables). User aliases keep the eager
    // path so their body diagnostics and primitive/union expansions are unchanged.
    if (!concrete_instantiation
        || (defer_concrete_library_aliases()
            && declaration_file_is_library_scoped(declaration, ctx)))
        && matches!(declaration, TypeDeclarationInfo::Alias(_))
        && let (Some(display), Some(arguments)) =
            (alias_display_name.as_ref(), reference_arguments.as_ref())
        && arguments.iter().all(|argument| !argument.is_unknown())
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

    if resolved.had_error {
        crate::program::record_degraded_resolution();
        if degraded_resolution_trace_enabled() {
            eprintln!(
                "{{\"degradedResolution\":\"{}\",\"file\":\"{}\"}}",
                declaration.declared_name(),
                ctx.file_name
            );
        }
    }

    let body_emitted_diagnostics = ctx.diagnostics().len() != diagnostics_before_body
        || ctx.utility_diagnostic_keys.len() != utility_keys_before_body;
    // A concrete instantiation that resolved cleanly is interned even when its body
    // re-entered an outer frame (`subtree_lowest_cycle < floor`). A clean re-entry
    // embeds the outer declaration as a lazy nominal `Type::Reference` (same id +
    // resolved arguments regardless of stack state), so the interned structural
    // expansion is context-free — the mutually-recursive user clusters (zod's
    // `$ZodType`/`$ZodTypeInternals`, `RawIssue`) were otherwise re-expanded from
    // every sibling reference, hundreds of thousands of times per file. A degraded
    // re-entry (bounded peel, illegal cycle) sets `had_error`/emits, which the
    // remaining guards still exclude, so a thin shape is never frozen program-wide.
    let cacheable = concrete_instantiation && !body_emitted_diagnostics && !resolved.had_error;

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

/// Opt-in (`SURGE_TRACE_TYPE_EXPANSION=1`) trace of degraded (`had_error`)
/// top-level named resolutions: which declaration degraded, in which file.
fn degraded_resolution_trace_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_TRACE_TYPE_EXPANSION").is_some())
}

/// Wraps a successfully-resolved generic *object* instantiation in a
/// lazy/nominal [`Type::Reference`] over its interned structural expansion, so it
/// carries nominal identity (declaration + resolved arguments) and a `Box<T>`
/// display form without forcing re-expansion at later use sites. Other cacheable
/// expansions (non-object bodies, display-less objects) are interned but returned
/// structurally; errored or argument-unresolved resolutions fall back to the
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
        // Any other cacheable expansion — a non-object body (union, nested
        // reference, primitive: `Exclude<…>`, `Omit<…>` whose body is itself a
        // reference) or a display-less object (a bare `$ZodType` reference whose
        // parameters all defaulted) — is interned under the same soundness
        // conditions, so the next reference to the same instantiation reuses it
        // instead of re-expanding the body (measured on zod: ~490k of 860k body
        // expansions were such cacheable re-expansions). The expansion is still
        // returned structurally — only the short-circuit reuse changes, not the
        // produced shape. An `unknown` result (or a union carrying one) may be a
        // degradation sentinel from a bounded lazy peel, so it is never frozen
        // program-wide.
        (_, Some(arguments), ty) if cacheable && !type_may_carry_degradation(ty) => {
            intern_instantiation(ctx, decl_key, &arguments, resolved.ty.clone());
            resolved
        }
        (display_name, _, _) => tag_generic_object_alias(resolved, display_name),
    }
}

/// Whether an expansion may embed the `unknown` degradation sentinel (a bounded
/// lazy peel returns `unknown` without setting `had_error`), making it unsafe to
/// intern program-wide even when the resolution was otherwise clean.
fn type_may_carry_degradation(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Union(union) => union
            .payload()
            .types
            .iter()
            .any(|member| matches!(member, Type::Unknown)),
        _ => false,
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
