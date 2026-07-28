//! Named-type resolution memoization and declaration resolution keys.

use super::*;

use std::path::Path;
use std::sync::Arc;

use surge_ts_types::{FunctionType, ResolveReference, Type, TypeReference};

use crate::context::{
    CanonicalTypeIdentity, CheckerContext, DeclarationEnvironmentHandle, DeclarationNamespace,
    DeclarationResolutionKey, DeclarationResolutionState, GenericInstantiationCacheEntry,
    InstantiationCacheEntry, InterfaceDeclarationTemplate, InterfaceEnvironmentIdentity,
    InterfaceInstantiationKey, InterfaceMemberDeclarationKind, InterfaceMemberDeclarationTemplate,
    InterfaceMemberInstantiationKey, InterfaceMethodOverloadGroupTemplate,
    InterfaceOverloadInstantiationKey, StableInterfaceDeclarationFragmentId,
    StableInterfaceDeclarationId, StableInterfaceMemberDeclarationId,
};
use crate::symbols::TypeDeclarationInfo;

pub(crate) fn type_declaration_resolution_key(
    declaration: &TypeDeclarationInfo,
) -> DeclarationResolutionKey {
    match declaration {
        TypeDeclarationInfo::Alias(alias) => alias
            .cached_resolution_key
            .get_or_init(|| declaration_resolution_key(&alias.file_name, &alias.name))
            .clone(),
        TypeDeclarationInfo::Interface(interface) => interface
            .cached_resolution_key
            .get_or_init(|| declaration_resolution_key(&interface.file_name, &interface.name))
            .clone(),
    }
}

pub(crate) fn declaration_resolution_key(file_name: &str, name: &str) -> DeclarationResolutionKey {
    DeclarationResolutionKey {
        file_name: canonical_declaration_file_name(file_name),
        name: Arc::from(name),
        namespace: DeclarationNamespace::Type,
    }
}

pub(crate) fn type_declaration_alias_id(
    declaration: &TypeDeclarationInfo,
    key: &DeclarationResolutionKey,
) -> Arc<str> {
    let build = || Arc::from(format!("{}\u{0}{}", key.file_name, key.name));
    match declaration {
        TypeDeclarationInfo::Alias(alias) => alias.cached_alias_id.get_or_init(build).clone(),
        TypeDeclarationInfo::Interface(interface) => {
            interface.cached_alias_id.get_or_init(build).clone()
        }
    }
}

pub(crate) fn get_cached_named_type_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    resolving: &[DeclarationResolutionKey],
) -> Option<ResolvedType> {
    let cache = ctx.resolved_named_types.lock().ok()?;

    match cache.get(key) {
        Some(DeclarationResolutionState::Resolved { ty, had_error }) => {
            crate::program::record_program_counter(|c| c.named_type_cache_hit_count += 1);
            Some(ResolvedType {
                ty: ty.clone(),
                had_error: *had_error,
            })
        }
        Some(DeclarationResolutionState::Resolving) => {
            if resolving.iter().any(|current| current == key) {
                None
            } else {
                Some(ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                })
            }
        }
        None => None,
    }
}

pub(crate) fn mark_named_type_resolution_in_progress(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
) {
    if let Ok(mut cache) = ctx.resolved_named_types.lock() {
        cache.insert(key.clone(), DeclarationResolutionState::Resolving);
    }
}

pub(crate) fn cache_named_type_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    resolved: &ResolvedType,
) {
    if let Ok(mut cache) = ctx.resolved_named_types.lock() {
        crate::program::record_program_counter(|c| c.named_type_cache_insert_count += 1);
        cache.insert(
            key.clone(),
            DeclarationResolutionState::Resolved {
                ty: resolved.ty.clone(),
                had_error: resolved.had_error,
            },
        );
    }
}

/// Upper bound on distinct instantiations memoized per generic declaration — a
/// defensive guard against a pathological declaration accumulating an unbounded
/// bucket that linear-search would have to scan. Sized for user utility aliases
/// (`Omit`, `Identity`, …), which accumulate hundreds of distinct argument
/// tuples on a large project; an entry evicted by a lower cap is re-expanded at
/// every remaining reference, which dominates checking time and peak memory
/// (measured on zod at the previous cap of 64).
const GENERIC_INSTANTIATION_BUCKET_CAP: usize = 4096;

/// Effective per-declaration bucket cap, overridable via
/// `SURGE_GENERIC_CACHE_BUCKET_CAP` for cache-bound experiments and the
/// bounded-vs-unbounded regression tests. Over-cap entries are simply not
/// cached (re-expanded on demand), so any cap produces identical diagnostics —
/// only time/memory change. Read once per process.
pub(crate) fn generic_instantiation_bucket_cap() -> usize {
    static CAP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("SURGE_GENERIC_CACHE_BUCKET_CAP")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(GENERIC_INSTANTIATION_BUCKET_CAP)
    })
}

pub(crate) fn get_persistent_generic_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
) -> Option<ResolvedType> {
    let resolved = if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_generic(&ctx.program_resolved_generic_types))
    {
        session
            .generic_lookup(key, arguments)
            .map(|(ty, had_error)| ResolvedType { ty, had_error })
    } else {
        ctx.program_resolved_generic_types
            .lock()
            .ok()
            .and_then(|cache| {
                cache.get(key)?.iter().find_map(|entry| {
                    (entry.arguments == arguments).then(|| ResolvedType {
                        ty: entry.ty.clone(),
                        had_error: entry.had_error,
                    })
                })
            })
    };
    crate::program::record_program_counter(|c| {
        if resolved.is_some() {
            c.generic_type_cache_hit_count += 1;
        } else {
            c.generic_type_cache_miss_count += 1;
        }
    });
    resolved
}

pub(crate) fn cache_persistent_generic_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: Vec<Type>,
    resolved: &ResolvedType,
) {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_generic(&ctx.program_resolved_generic_types))
    {
        session.generic_insert(
            key,
            arguments,
            resolved.ty.clone(),
            resolved.had_error,
            generic_instantiation_bucket_cap(),
        );
        return;
    }
    if let Ok(mut cache) = ctx.program_resolved_generic_types.lock() {
        let bucket = cache.entry(key.clone()).or_default();
        if bucket.iter().any(|entry| entry.arguments == arguments) {
            return;
        }
        if bucket.len() >= generic_instantiation_bucket_cap() {
            crate::program::record_program_counter(|c| c.generic_type_cache_capped_count += 1);
            return;
        }
        crate::program::record_program_counter(|c| c.generic_type_cache_insert_count += 1);
        bucket.push(GenericInstantiationCacheEntry {
            arguments,
            ty: resolved.ty.clone(),
            had_error: resolved.had_error,
        });
    }
}

pub(crate) fn canonical_declaration_file_name(file_name: &str) -> Arc<str> {
    crate::paths::canonicalize_if_exists_arc(Path::new(file_name))
}

/// Resolver for a lazy [`Type::Reference`] that resolves to an already-computed,
/// program-wide-shared structural expansion. The expansion is computed once per
/// unique instantiation by [`intern_instantiation`] and shared via `Arc`, so
/// resolving the reference never re-expands the declaration body.
#[derive(Debug)]
struct InternedInstantiation {
    resolved: Arc<Type>,
}

impl ResolveReference for InternedInstantiation {
    fn resolve(&self) -> Type {
        (*self.resolved).clone()
    }

    fn resolve_arc(&self) -> Arc<Type> {
        self.resolved.clone()
    }

    fn peek_resolved(&self) -> Option<Arc<Type>> {
        Some(self.resolved.clone())
    }
}

/// Maximum nesting of in-flight lazy peels before a deeper one degrades to
/// `unknown`. Real reference chains a consumer forces (an event-handler param, an
/// inheritance chain) stay well under this; the bound only trips on a runaway
/// library `extends` cluster.
const MAX_LAZY_PEEL_DEPTH: usize = 24;

/// How many times one generic declaration may appear in the in-flight peel stack
/// before a deeper re-entry degrades to `unknown`. Bounds the mutually-recursive
/// library clusters (`A<X>` → `A<f(X)>` → …) while still allowing modest, genuine
/// self-nesting.
const MAX_SAME_DECLARATION_PEELS: usize = 3;

thread_local! {
    /// Instantiations whose lazy body is currently being expanded on this thread.
    /// A mutually-recursive library cluster (`HTMLElement` → `Element` → … , the
    /// iterator/typed-array clusters) can peel back into an instantiation while
    /// expanding it; this stack breaks that re-entry with `unknown` instead of
    /// recursing forever. Keyed by declaration + resolved arguments.
    static LAZY_PEEL_STACK: std::cell::RefCell<Vec<(DeclarationResolutionKey, Vec<Type>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Resolver for a deferred library [`Type::Reference`]. Unlike
/// [`InternedInstantiation`] (which holds an already-expanded structural type),
/// this expands the declaration body *on first peel* — one level deep, since the
/// nested named types it references resolve to their own deferred references. This
/// is what keeps resolving a type argument such as `HTMLElement` from eagerly
/// pulling the whole DOM/iterator graph: the bulk shape is materialised only for
/// the instantiations a consumer actually inspects.
struct LazyInstantiation {
    environment: DeclarationEnvironmentHandle,
    /// The type-declaration scope installed where this reference was created.
    /// The compact environment may be captured before the declaring module
    /// scope is installed, so a peel through it alone cannot see the
    /// declaring module's siblings (a namespace member registered without its
    /// own `resolution_scope`, like the bare `JSX.IntrinsicElements` dual key,
    /// would resolve every member reference to `unknown`). Re-installing the
    /// creation-time scope restores the lexical environment the reference was
    /// formed in; a declaration carrying its own scope still overrides it.
    creation_scope: Option<Arc<crate::symbols::TypeDeclarationScope>>,
    decl: crate::symbols::TypeDeclarationHandle,
    decl_key: DeclarationResolutionKey,
    type_arguments: Vec<surge_ts_syntax::ParsedType>,
    resolved_arguments: Vec<Type>,
    substitution: TypeParameterSubstitution,
    display: Arc<str>,
    /// Weak so a back-edge reference embedded in an interned expansion cannot
    /// keep its own container alive: memoizing the containing `Arc<Type>`
    /// strongly forms a reference↔expansion cycle that survives even after the
    /// program caches are cleared. The strong ref lives in
    /// `program_instantiations`; if it is gone the resolve falls through to a
    /// fresh peel.
    memo: std::sync::OnceLock<std::sync::Weak<Type>>,
}

struct LazyDeclarationAnnotation {
    environment: DeclarationEnvironmentHandle,
    creation_scope: Option<Arc<crate::symbols::TypeDeclarationScope>>,
    key: DeclarationResolutionKey,
    display: Arc<str>,
    annotation: surge_ts_syntax::ParsedType,
    signature_component: Option<LazySignatureComponent>,
    signature_environment: Option<LazySignatureEnvironment>,
    memo: std::sync::OnceLock<std::sync::Weak<Type>>,
    /// A degraded (`had_error`/unknown) resolution is never interned into the
    /// shared caches (that would violate the no-degraded-results-program-wide
    /// rule), so the weak `memo` has no keeper and every read would re-run the
    /// full failed resolution — hot on value annotations read once per use
    /// site. Pinning the FIRST answer per annotation instance both bounds the
    /// cost and matches the eager collector's resolve-once semantics.
    degraded_memo: std::sync::OnceLock<Arc<Type>>,
}

#[derive(Clone, Copy)]
pub(crate) enum LazySignatureComponent {
    Parameter(usize),
    Return,
    ThisParameter,
    #[allow(dead_code)]
    TypePredicate,
}

#[derive(Clone)]
pub(crate) struct LazySignatureEnvironment {
    type_parameters: Arc<[surge_ts_syntax::ParsedTypeParameter]>,
    substitution: Arc<TypeParameterSubstitution>,
}

impl LazySignatureEnvironment {
    pub(crate) fn new(type_parameters: &[surge_ts_syntax::ParsedTypeParameter]) -> Option<Self> {
        if type_parameters.is_empty() {
            return None;
        }
        crate::program::record_program_counter(|c| {
            c.lazy_signature_environment_create_count += 1;
            c.lazy_signature_environment_handle_size_bytes =
                std::mem::size_of::<LazySignatureEnvironment>() as u64;
        });
        let mut substitution = TypeParameterSubstitution::new();
        for type_parameter in type_parameters {
            substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
        }
        Some(Self {
            type_parameters: Arc::from(type_parameters),
            substitution: Arc::new(substitution),
        })
    }
}

impl LazySignatureComponent {
    fn identity(self) -> String {
        match self {
            Self::Parameter(index) => format!("parameter-{index}"),
            Self::Return => "return".to_string(),
            Self::ThisParameter => "this-parameter".to_string(),
            Self::TypePredicate => "type-predicate".to_string(),
        }
    }

    fn peel_reason(self) -> crate::program::DtsExpansionReason {
        match self {
            Self::Parameter(_) => crate::program::DtsExpansionReason::SignatureParameter,
            Self::Return => crate::program::DtsExpansionReason::SignatureReturn,
            Self::ThisParameter => crate::program::DtsExpansionReason::SignatureThisParameter,
            Self::TypePredicate => crate::program::DtsExpansionReason::SignatureTypePredicate,
        }
    }
}

impl ResolveReference for LazyDeclarationAnnotation {
    fn resolve(&self) -> Type {
        (*self.resolve_arc()).clone()
    }

    fn resolve_arc(&self) -> Arc<Type> {
        let resolve = || self.resolve_arc_inner();
        let Some(component) = self.signature_component else {
            return resolve();
        };
        if crate::program::current_dts_expansion_reason()
            == crate::program::DtsExpansionReason::Other
        {
            crate::program::with_dts_expansion_reason(component.peel_reason(), resolve)
        } else {
            resolve()
        }
    }

    fn retains_resolution_context(&self) -> bool {
        false
    }

    fn supports_program_canonicalization(&self) -> bool {
        true
    }

    fn program_canonicalization_discriminator(&self) -> u64 {
        self.environment.canonicalization_discriminator()
    }

    fn captured_census(&self) -> surge_ts_types::ResolverCaptureCensus {
        let mut own_bytes = std::mem::size_of::<Self>() as u64
            + self.annotation.estimated_heap_bytes()
            + self.key.name.len() as u64;
        let mut shared_captures = Vec::new();
        if let Some(environment) = &self.signature_environment {
            shared_captures.push((
                environment.type_parameters.as_ptr() as usize,
                environment
                    .type_parameters
                    .iter()
                    .map(surge_ts_syntax::ParsedTypeParameter::estimated_heap_bytes)
                    .sum(),
            ));
            shared_captures.extend(environment.substitution.census_shared_captures());
            own_bytes += std::mem::size_of::<LazySignatureEnvironment>() as u64;
        }
        surge_ts_types::ResolverCaptureCensus {
            own_bytes,
            shared_captures,
        }
    }

    fn peek_resolved(&self) -> Option<Arc<Type>> {
        self.memo.get().and_then(std::sync::Weak::upgrade)
    }
}

impl LazyDeclarationAnnotation {
    fn resolve_arc_inner(&self) -> Arc<Type> {
        crate::program::record_lazy_reference_peel_start(&self.key);
        if self.signature_component.is_some() {
            crate::program::record_program_counter(|c| c.lazy_signature_materialization_count += 1);
        }
        if let Some(resolved) = self.memo.get().and_then(std::sync::Weak::upgrade) {
            crate::program::record_program_counter(|c| c.lazy_reference_memo_hit_count += 1);
            if self.signature_component.is_some() {
                crate::program::record_program_counter(|c| {
                    c.signature_materialization_cache_hit_count += 1
                });
            }
            return resolved;
        }
        if let Some(degraded) = self.degraded_memo.get() {
            crate::program::record_program_counter(|c| c.lazy_reference_memo_hit_count += 1);
            return degraded.clone();
        }
        let Some(ctx) = self.environment.checker_context() else {
            return Arc::new(Type::Unknown);
        };
        if let Some(entry) = lookup_instantiation(&ctx, &self.key, &[]) {
            crate::program::record_program_counter(|c| c.lazy_reference_interner_hit_count += 1);
            if self.signature_component.is_some() {
                crate::program::record_program_counter(|c| {
                    c.signature_materialization_cache_hit_count += 1
                });
            }
            let _ = self.memo.set(Arc::downgrade(&entry.resolved));
            return entry.resolved;
        }
        if self.signature_component.is_some() {
            crate::program::record_program_counter(|c| {
                c.signature_materialization_cache_miss_count += 1
            });
        }

        let before = crate::program::type_creation_snapshot();
        crate::program::record_lazy_reference_expansion_start(
            &self.key,
            &ctx.file_name,
            &self.display,
            0,
        );
        let mut ctx = Box::new(ctx);
        ctx.set_file_name(self.key.file_name.as_ref().to_string());
        if self.creation_scope.is_some() {
            ctx.type_declaration_scope = self.creation_scope.clone();
        }
        if let Some(environment) = &self.signature_environment {
            ctx.push_type_parameter_scope(&environment.type_parameters, None);
        }
        let empty_substitution = TypeParameterSubstitution::new();
        let substitution = self
            .signature_environment
            .as_ref()
            .map_or(&empty_substitution, |environment| {
                environment.substitution.as_ref()
            });
        let resolved = resolve_parsed_type(
            self.annotation.clone(),
            &mut ctx,
            &mut Vec::new(),
            substitution,
        );
        let had_error = resolved.had_error;
        // Signature components historically peel to the structural type. A
        // VALUE annotation (no signature component) must NOT: eager collection
        // stored `map_parsed_type`'s output verbatim, leaving named references
        // nominal so members expand through the live-context peel path — an
        // eager peel here would bake a snapshot of the recovered environment's
        // expansion into the symbol and drift from the eager shape.
        let resolved = Arc::new(match resolved.ty {
            Type::Reference(reference) if self.signature_component.is_some() => {
                reference.resolve().peeled()
            }
            ty => ty,
        });
        if let Some(filter) = lazy_value_trace_filter()
            && self.key.name.contains(filter)
        {
            let diagnostics: Vec<String> = ctx
                .diagnostics
                .iter()
                .take(4)
                .map(|d| format!("{}:{}", d.code, d.message.chars().take(90).collect::<String>()))
                .collect();
            eprintln!(
                "[lazy-value] FORCE {} had_error={had_error} diags={:?} ty={}",
                self.key.name,
                diagnostics,
                lazy_value_trace_shape(&resolved),
            );
        }
        if had_error || resolved.is_unknown() {
            crate::program::note_expansion_degradation();
            crate::program::record_program_counter(|c| {
                c.lazy_reference_degraded_expansion_count += 1;
                if self.signature_component.is_some() {
                    c.degraded_signature_expansion_count += 1;
                }
            });
            if self.signature_component.is_some() {
                crate::program::record_degraded_signature_expansion(&self.key);
            }
            let _ = self.degraded_memo.set(resolved.clone());
            return resolved;
        }
        let resolved = intern_instantiation(&ctx, &self.key, &[], (*resolved).clone());
        let _ = self.memo.set(Arc::downgrade(&resolved));
        crate::program::record_lazy_reference_expansion(
            &self.key,
            &ctx.file_name,
            &self.display,
            0,
            before,
        );
        if self.signature_component.is_some() {
            crate::program::record_program_counter(|c| c.clean_signature_expansion_count += 1);
        }
        resolved
    }
}

pub(crate) fn make_lazy_signature_annotation_reference(
    ctx: &mut CheckerContext,
    declaration_name: &str,
    declaration_start: usize,
    component: LazySignatureComponent,
    annotation: surge_ts_syntax::ParsedType,
    signature_environment: Option<LazySignatureEnvironment>,
) -> Type {
    let display: Arc<str> = Arc::from(parsed_annotation_display(&annotation));
    let component_identity = component.identity();
    let key = DeclarationResolutionKey {
        file_name: canonical_declaration_file_name(&ctx.file_name),
        name: Arc::from(format!(
            "signature {declaration_name}@{declaration_start}:{component_identity}"
        )),
        namespace: DeclarationNamespace::Type,
    };
    crate::program::record_lazy_reference_created(&key);
    crate::program::record_program_counter(|c| {
        match component {
            LazySignatureComponent::Parameter(_)
            | LazySignatureComponent::ThisParameter
            | LazySignatureComponent::TypePredicate => {
                c.lazy_signature_parameter_annotation_create_count += 1
            }
            LazySignatureComponent::Return => c.lazy_signature_return_annotation_create_count += 1,
        }
        c.lazy_signature_annotation_handle_size_bytes =
            std::mem::size_of::<LazyDeclarationAnnotation>() as u64;
        c.lazy_signature_parameter_slot_size_bytes = std::mem::size_of::<Type>() as u64;
        c.lazy_signature_estimated_shallow_retained_bytes +=
            (std::mem::size_of::<LazyDeclarationAnnotation>()
                + std::mem::size_of::<surge_ts_types::TypeReference>()) as u64;
        if signature_environment.is_some() {
            c.lazy_signature_environment_reference_count += 1;
        }
    });
    let id = format!(
        "{}\u{0}signature-annotation\u{0}{declaration_name}\u{0}{declaration_start}\u{0}{component_identity}",
        key.file_name
    );
    let environment = ctx.declaration_environment();
    let creation_scope = ctx.type_declaration_scope.clone();
    Type::Reference(TypeReference::new(
        id,
        display.clone(),
        Vec::new(),
        Arc::new(LazyDeclarationAnnotation {
            environment,
            creation_scope,
            key,
            display,
            annotation,
            signature_component: Some(component),
            signature_environment,
            memo: std::sync::OnceLock::new(),
            degraded_memo: std::sync::OnceLock::new(),
        }),
    ))
}

/// A lazy reference for a library declaration's VALUE annotation
/// (`declare const x: T` in a dependency `.d.ts`): the annotation maps on
/// first read under the captured declaration environment instead of eagerly
/// during exportable-value collection. Unlike the signature variant, the
/// resolver returns the mapped type UNPEELED (see `resolve_arc_inner`) so the
/// symbol carries exactly the shape eager `map_parsed_type` would have
/// produced — nested named references stay nominal and expand through the
/// normal live-context peel path.
pub(crate) fn make_lazy_value_annotation_reference(
    ctx: &mut CheckerContext,
    declaration_name: &str,
    declaration_start: usize,
    annotation: surge_ts_syntax::ParsedType,
) -> Type {
    let display: Arc<str> = Arc::from(parsed_annotation_display(&annotation));
    let key = DeclarationResolutionKey {
        file_name: canonical_declaration_file_name(&ctx.file_name),
        name: Arc::from(format!("value {declaration_name}@{declaration_start}")),
        namespace: DeclarationNamespace::Type,
    };
    crate::program::record_lazy_reference_created(&key);
    if let Some(filter) = lazy_value_trace_filter()
        && key.name.contains(filter)
    {
        eprintln!("[lazy-value] CREATE {} file={}", key.name, key.file_name);
    }
    let id = format!(
        "{}\u{0}value-annotation\u{0}{declaration_name}\u{0}{declaration_start}",
        key.file_name
    );
    let environment = ctx.declaration_environment();
    let creation_scope = ctx.type_declaration_scope.clone();
    Type::Reference(TypeReference::new(
        id,
        display.clone(),
        Vec::new(),
        Arc::new(LazyDeclarationAnnotation {
            environment,
            creation_scope,
            key,
            display,
            annotation,
            signature_component: None,
            signature_environment: None,
            memo: std::sync::OnceLock::new(),
            degraded_memo: std::sync::OnceLock::new(),
        }),
    ))
}

/// Opt-in probe filter (`SURGE_LAZY_VALUE_TRACE=<substr>`), read once — the
/// trace sites sit on resolution paths where per-call `getenv` is prohibited.
pub(crate) fn lazy_value_trace_filter() -> Option<&'static str> {
    static FILTER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    FILTER
        .get_or_init(|| std::env::var("SURGE_LAZY_VALUE_TRACE").ok())
        .as_deref()
}

/// Debug shape for the `SURGE_LAZY_VALUE_TRACE` probe: variant + display +
/// (for objects / peeled references) the member-name list.
pub(crate) fn lazy_value_trace_shape(ty: &Type) -> String {
    fn describe(ty: &Type, force: bool) -> String {
        match ty {
            Type::Object(object) => {
                let mut names: Vec<&str> =
                    object.properties.keys().map(|k| k.as_ref()).collect();
                names.sort_unstable();
                format!(
                    "Object{{{} props: {}}} call={}",
                    names.len(),
                    names.join(","),
                    object.call_signature.is_some(),
                )
            }
            Type::Reference(reference) => {
                if force {
                    let peeled = reference.resolve().peeled();
                    format!(
                        "Reference({}) -> {}",
                        reference.display,
                        describe(&peeled, false)
                    )
                } else {
                    format!("Reference({})", reference.display)
                }
            }
            other => format!("{other:?}").chars().take(120).collect(),
        }
    }
    describe(ty, true)
}

fn parsed_annotation_display(annotation: &surge_ts_syntax::ParsedType) -> String {
    use surge_ts_syntax::ParsedType;

    match annotation {
        ParsedType::String => "string".to_string(),
        ParsedType::Number => "number".to_string(),
        ParsedType::Boolean => "boolean".to_string(),
        ParsedType::BigInt => "bigint".to_string(),
        ParsedType::Symbol => "symbol".to_string(),
        ParsedType::Undefined => "undefined".to_string(),
        ParsedType::Void => "void".to_string(),
        ParsedType::Any => "any".to_string(),
        ParsedType::Unknown | ParsedType::UnknownKeyword => "unknown".to_string(),
        ParsedType::Never => "never".to_string(),
        ParsedType::StringLiteral(value) => format!("\"{value}\""),
        ParsedType::NumberLiteral(value) => value.clone(),
        ParsedType::BooleanLiteral(value) => value.to_string(),
        ParsedType::Named(named) => {
            if named.type_arguments.is_empty() {
                named.name.clone()
            } else {
                format!(
                    "{}<{}>",
                    named.name,
                    named
                        .type_arguments
                        .iter()
                        .map(parsed_annotation_display)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        ParsedType::Array(element) => {
            let display = parsed_annotation_display(element);
            if matches!(
                element.as_ref(),
                ParsedType::Union(_)
                    | ParsedType::Intersection(_)
                    | ParsedType::Function(_)
                    | ParsedType::Conditional(_)
            ) {
                format!("({display})[]")
            } else {
                format!("{display}[]")
            }
        }
        ParsedType::Tuple(elements) => format!(
            "[{}]",
            elements
                .iter()
                .map(parsed_annotation_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ParsedType::Union(members) => members
            .iter()
            .map(parsed_annotation_display)
            .collect::<Vec<_>>()
            .join(" | "),
        ParsedType::Intersection(members) => members
            .iter()
            .map(parsed_annotation_display)
            .collect::<Vec<_>>()
            .join(" & "),
        ParsedType::TypeOf(query) => {
            let suffix = query
                .members
                .iter()
                .map(|member| format!(".{member}"))
                .collect::<String>();
            format!("typeof {}{suffix}", query.name)
        }
        ParsedType::KeyOf(inner) => format!("keyof {}", parsed_annotation_display(inner)),
        ParsedType::IndexedAccess(indexed) => format!(
            "{}[{}]",
            parsed_annotation_display(&indexed.object_type),
            parsed_annotation_display(&indexed.index_type)
        ),
        ParsedType::Function(function) => {
            let type_parameters = if function.type_parameters.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    function
                        .type_parameters
                        .iter()
                        .map(|parameter| {
                            let mut display = parameter.name.clone();
                            if let Some(constraint) = &parameter.constraint {
                                display.push_str(" extends ");
                                display.push_str(&parsed_annotation_display(constraint));
                            }
                            if let Some(default_type) = &parameter.default_type {
                                display.push_str(" = ");
                                display.push_str(&parsed_annotation_display(default_type));
                            }
                            display
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| {
                    let rest = if parameter.rest { "..." } else { "" };
                    let name = parameter.name.as_deref().unwrap_or("arg");
                    let optional = if parameter.optional { "?" } else { "" };
                    format!(
                        "{rest}{name}{optional}: {}",
                        parsed_annotation_display(&parameter.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{type_parameters}({parameters}) => {}",
                parsed_annotation_display(&function.return_type)
            )
        }
        ParsedType::Object(object) => {
            let mut members = object
                .properties
                .iter()
                .map(|property| {
                    let optional = if property.optional { "?" } else { "" };
                    format!(
                        "{}{optional}: {}",
                        property.name,
                        parsed_annotation_display(&property.ty)
                    )
                })
                .collect::<Vec<_>>();
            if let Some(call) = &object.call_signature {
                members.push(parsed_annotation_display(&ParsedType::Function(
                    std::sync::Arc::new(call.as_ref().clone()),
                )));
            }
            format!("{{ {} }}", members.join("; "))
        }
        ParsedType::Mapped(mapped) => {
            let optional = if mapped.optional { "?" } else { "" };
            format!(
                "{{ [{} in {}]{optional}: {} }}",
                mapped.key_name,
                parsed_annotation_display(&mapped.constraint),
                parsed_annotation_display(&mapped.value_type)
            )
        }
        ParsedType::Conditional(conditional) => format!(
            "{} extends {} ? {} : {}",
            parsed_annotation_display(&conditional.check_type),
            parsed_annotation_display(&conditional.extends_type),
            parsed_annotation_display(&conditional.true_type),
            parsed_annotation_display(&conditional.false_type)
        ),
        ParsedType::TemplateLiteral(template) => {
            let mut display = String::from("`");
            for (index, quasi) in template.quasis.iter().enumerate() {
                display.push_str(quasi);
                if let Some(interpolation) = template.interpolations.get(index) {
                    display.push_str("${");
                    display.push_str(&parsed_annotation_display(interpolation));
                    display.push('}');
                }
            }
            display.push('`');
            display
        }
        ParsedType::Infer(name) => format!("infer {name}"),
    }
}

impl ResolveReference for LazyInstantiation {
    fn resolve(&self) -> Type {
        (*self.resolve_arc()).clone()
    }

    fn captured_census(&self) -> surge_ts_types::ResolverCaptureCensus {
        let own_bytes = std::mem::size_of::<Self>() as u64
            + self
                .type_arguments
                .iter()
                .map(surge_ts_syntax::ParsedType::estimated_heap_bytes)
                .sum::<u64>()
            + (self.resolved_arguments.len() * std::mem::size_of::<Type>()) as u64
            + self.decl_key.name.len() as u64;
        surge_ts_types::ResolverCaptureCensus {
            own_bytes,
            shared_captures: self.substitution.census_shared_captures(),
        }
    }

    fn peek_resolved(&self) -> Option<Arc<Type>> {
        self.memo.get().and_then(std::sync::Weak::upgrade)
    }

    fn resolve_arc(&self) -> Arc<Type> {
        crate::program::record_lazy_reference_peel_start(&self.decl_key);
        if let Some(memoized) = self.memo.get().and_then(std::sync::Weak::upgrade) {
            crate::program::record_program_counter(|c| c.lazy_reference_memo_hit_count += 1);
            return memoized;
        }
        let Some(ctx) = self.environment.checker_context() else {
            return Arc::new(Type::Unknown);
        };
        // A peel of the same instantiation elsewhere may have already interned it.
        match lookup_instantiation_probe(&ctx, &self.decl_key, &self.resolved_arguments) {
            crate::speculative::InstantiationProbe::Hit(entry) => {
                crate::program::record_program_counter(|c| {
                    c.lazy_reference_interner_hit_count += 1
                });
                let _ = self.memo.set(Arc::downgrade(&entry.resolved));
                return entry.resolved;
            }
            // Resolution-deferred: this instantiation is owned by an earlier
            // not-yet-committed serial position (a replay reservation). Serial
            // checking here would have *hit* that publisher's interned expansion,
            // so expanding the body now would over-recurse and intern a spurious
            // structural sub-instantiation. Return the nominal reference instead —
            // un-memoized, before touching the peel stack — and let the replay's
            // file-check attempt be discarded and requeued once the publisher
            // commits (see `crate::replay`). The deferral is control flow, not a
            // `Type`: the probe returns `Deferred` at most once per key per attempt
            // (`WorkerOverlay::deferred_once`), so forcing this nominal later
            // resolves the key as a normal miss and terminates `Type::peeled`.
            crate::speculative::InstantiationProbe::Deferred => {
                let mut ctx = ctx;
                return Arc::new(make_recursive_cycle_reference(
                    &mut ctx,
                    &self.display,
                    self.decl.clone(),
                    self.decl_key.clone(),
                    self.type_arguments.clone(),
                    Some(&self.resolved_arguments),
                    &self.substitution,
                ));
            }
            crate::speculative::InstantiationProbe::Miss => {}
        }

        let guard_key = (self.decl_key.clone(), self.resolved_arguments.clone());
        let blocked = LAZY_PEEL_STACK.with(|stack| {
            let stack = stack.borrow();
            if stack.len() >= MAX_LAZY_PEEL_DEPTH {
                return true;
            }
            // Exact re-entry is a true cycle. The per-declaration count also stops a
            // chain that re-enters the *same* generic declaration with ever-changing
            // arguments (`A<X>` → `A<f(X)>` → …, as the mutually-recursive lib
            // typed-array/iterator clusters do), which the exact-key check misses
            // because every key differs. A few repeats are allowed for legitimate
            // self-nesting before the back-edge degrades to `unknown`.
            let same_decl = stack
                .iter()
                .filter(|entry| entry.0 == self.decl_key)
                .count();
            same_decl >= MAX_SAME_DECLARATION_PEELS || stack.iter().any(|entry| *entry == guard_key)
        });
        if blocked {
            crate::program::note_expansion_degradation();
            crate::program::record_program_counter(|c| c.lazy_reference_blocked_count += 1);
            return Arc::new(Type::Unknown);
        }
        let creation_before = crate::program::type_creation_snapshot();
        crate::program::record_lazy_reference_expansion_start(
            &self.decl_key,
            &ctx.file_name,
            &self.display,
            LAZY_PEEL_STACK.with(|stack| stack.borrow().len()),
        );
        LAZY_PEEL_STACK.with(|stack| stack.borrow_mut().push(guard_key.clone()));

        // Box the working context so a nested peel keeps the per-frame stack small
        // — the struct is large and a deep (but bounded) library `extends` chain
        // would otherwise overflow the stack with on-stack clones.
        let mut ctx = Box::new(ctx);
        if self.creation_scope.is_some() {
            ctx.type_declaration_scope = self.creation_scope.clone();
        }
        let mut resolving = Vec::new();
        let resolved = match self.decl.get() {
            TypeDeclarationInfo::Alias(alias) => resolve_type_alias(
                alias,
                self.decl.clone(),
                self.type_arguments.clone(),
                None,
                &mut ctx,
                &mut resolving,
                &self.substitution,
                Some(&self.resolved_arguments),
            ),
            TypeDeclarationInfo::Interface(interface) => resolve_interface(
                interface,
                self.decl.clone(),
                self.type_arguments.clone(),
                &mut ctx,
                &mut resolving,
                &self.substitution,
                Some(&self.resolved_arguments),
            ),
        };

        LAZY_PEEL_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            if let Some(position) = stack.iter().rposition(|entry| *entry == guard_key) {
                stack.remove(position);
            }
        });

        // A degraded peel (a member that hit an incomplete scope or a bounded
        // re-entry and collapsed to `unknown`) must not be interned: the program
        // instantiation cache is first-wins and program-wide, so a degraded shape
        // computed under a transient incomplete scope (e.g. the binding/signature
        // pass, before `module_scope_by_file` is populated) would permanently
        // shadow the correct expansion every later peel produces. Return the
        // degraded shape transiently instead, leaving the cache for a clean peel to
        // populate, and do not memoize it on this reference.
        if resolved.had_error {
            crate::program::note_expansion_degradation();
            crate::program::record_degraded_resolution();
            crate::program::record_program_counter(|c| {
                c.lazy_reference_degraded_expansion_count += 1
            });
            return Arc::new(resolved.ty);
        }

        // The eager named-type path tags the resolved object with its declaration
        // name so diagnostics display the nominal form (`Client`, `Box<string>`)
        // instead of the structural expansion. A library-scoped interface is
        // routed here instead, so attach the same display name on peel; otherwise
        // a peeled reference (e.g. the TS2741 target type) renders structurally.
        // Display-only: `alias_name` is excluded from equality, so assignability
        // is unchanged.
        let resolved_ty = match resolved.ty {
            Type::Object(object)
                if object.alias_name.is_none() && !object.properties.is_empty() =>
            {
                Type::Object(object.with_alias_name(Arc::clone(&self.display)))
            }
            other => other,
        };

        let interned =
            intern_instantiation(&ctx, &self.decl_key, &self.resolved_arguments, resolved_ty);
        let _ = self.memo.set(Arc::downgrade(&interned));
        crate::program::record_lazy_reference_expansion(
            &self.decl_key,
            &ctx.file_name,
            &self.display,
            LAZY_PEEL_STACK.with(|stack| stack.borrow().len()),
            creation_before,
        );
        interned
    }

    fn retains_resolution_context(&self) -> bool {
        false
    }

    fn supports_program_canonicalization(&self) -> bool {
        true
    }

    fn program_canonicalization_discriminator(&self) -> u64 {
        self.environment.canonicalization_discriminator()
    }
}

/// Builds a deferred library [`Type::Reference`] whose structural body is expanded
/// lazily on peel (see [`LazyInstantiation`]). `resolved_arguments` carry the
/// nominal identity and display; `decl`/`substitution` drive the one-level
/// expansion when forced.
pub(crate) fn make_lazy_type_reference(
    ctx: &mut CheckerContext,
    reference_id: &str,
    display: &str,
    decl: crate::symbols::TypeDeclarationHandle,
    decl_key: DeclarationResolutionKey,
    type_arguments: Vec<surge_ts_syntax::ParsedType>,
    resolved_arguments: Vec<Type>,
    substitution: TypeParameterSubstitution,
) -> Type {
    crate::program::record_lazy_reference_created(&decl_key);
    let environment = ctx.declaration_environment();
    let creation_scope = ctx.type_declaration_scope.clone();
    Type::Reference(TypeReference::new(
        reference_id.to_string(),
        display.to_string(),
        resolved_arguments.clone(),
        Arc::new(LazyInstantiation {
            environment,
            creation_scope,
            decl,
            decl_key,
            type_arguments,
            resolved_arguments,
            substitution,
            display: Arc::from(display),
            memo: std::sync::OnceLock::new(),
        }),
    ))
}

/// Builds the lazy nominal [`Type::Reference`] a recursive declaration's
/// self-edge resolves to when a resolution cycle is detected (see
/// `resolve_type_alias` / `resolve_interface`). It carries the declaration's
/// nominal identity and defers re-expansion to [`LazyInstantiation`], so forcing
/// the back-edge peels one level to the real recursive shape (bounded by the lazy
/// peel stack) instead of collapsing to `unknown`.
pub(crate) fn make_recursive_cycle_reference(
    ctx: &mut CheckerContext,
    name: &str,
    handle: crate::symbols::TypeDeclarationHandle,
    decl_key: DeclarationResolutionKey,
    type_arguments: Vec<surge_ts_syntax::ParsedType>,
    pre_resolved_arguments: Option<&[Type]>,
    substitution: &TypeParameterSubstitution,
) -> Type {
    let reference_id = format!("{}\u{0}{}", decl_key.file_name, decl_key.name);
    let resolved_arguments = pre_resolved_arguments
        .map(<[Type]>::to_vec)
        .unwrap_or_default();
    make_lazy_type_reference(
        ctx,
        &reference_id,
        name,
        handle,
        decl_key,
        type_arguments,
        resolved_arguments,
        substitution.clone_with_reason(TypeCopyReason::SubstitutionUnchanged),
    )
}

/// Interns the structural expansion of `key` at `arguments`, returning the
/// shared `Arc<Type>`. On a hit the previously-expanded shape is returned and
/// `structural` is discarded, so each unique instantiation expands at most once.
pub(crate) fn intern_instantiation(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
    structural: Type,
) -> Arc<Type> {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_instantiations(&ctx.program_instantiations))
    {
        return session.instantiation_intern(
            key,
            arguments,
            structural,
            generic_instantiation_bucket_cap(),
        );
    }
    let Ok(mut cache) = ctx.program_instantiations.lock() else {
        return Arc::new(structural);
    };
    let bucket = cache.entry(key.clone()).or_default();
    if let Some(entry) = bucket.iter().find(|entry| entry.arguments == arguments) {
        crate::program::record_program_counter(|c| c.instantiation_intern_hit_count += 1);
        return entry.resolved.clone();
    }
    let resolved = Arc::new(structural);
    if bucket.len() < generic_instantiation_bucket_cap() {
        crate::program::record_program_counter(|c| c.instantiation_intern_insert_count += 1);
        bucket.push(InstantiationCacheEntry {
            arguments: arguments.to_vec(),
            resolved: resolved.clone(),
        });
    } else {
        crate::program::record_program_counter(|c| c.instantiation_intern_capped_count += 1);
    }
    resolved
}

/// Looks up a previously-interned instantiation without expanding anything.
pub(crate) fn lookup_instantiation(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
) -> Option<InstantiationCacheEntry> {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_instantiations(&ctx.program_instantiations))
    {
        return session.instantiation_lookup(key, arguments);
    }
    let cache = ctx.program_instantiations.lock().ok()?;
    cache
        .get(key)?
        .iter()
        .find(|entry| entry.arguments == arguments)
        .cloned()
}

/// Deferral-aware instantiation lookup for the lazy peel: distinguishes a
/// genuine miss from a deferral (the key is owned by an earlier not-yet-committed
/// replay publisher). Only a live deferring replay session ever returns
/// `Deferred`; every other context sees `Hit`/`Miss` exactly as
/// [`lookup_instantiation`].
pub(crate) fn lookup_instantiation_probe(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    arguments: &[Type],
) -> crate::speculative::InstantiationProbe {
    use crate::speculative::InstantiationProbe;
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_instantiations(&ctx.program_instantiations))
    {
        return session.instantiation_probe(key, arguments);
    }
    match ctx.program_instantiations.lock().ok().and_then(|cache| {
        cache
            .get(key)
            .and_then(|bucket| bucket.iter().find(|entry| entry.arguments == arguments))
            .cloned()
    }) {
        Some(entry) => InstantiationProbe::Hit(entry),
        None => InstantiationProbe::Miss,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterfaceCacheSkipReason {
    Disabled,
    UnstableDeclaration,
    UnresolvedTypeArgument,
    UnsupportedTypeArgument,
}

pub(crate) fn physical_interface_cache_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SURGE_DISABLE_PHYSICAL_INTERFACE_CACHE").as_deref() != Ok("1")
    })
}

pub(crate) fn physical_interface_member_cache_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("SURGE_DISABLE_PHYSICAL_INTERFACE_MEMBER_CACHE").as_deref() != Ok("1")
    })
}

pub(crate) fn physical_interface_declaration_template(
    ctx: &CheckerContext,
    interface: &crate::symbols::InterfaceInfo,
    declaration: &StableInterfaceDeclarationId,
) -> Option<Arc<InterfaceDeclarationTemplate>> {
    let session = crate::speculative::active_check_session()
        .filter(|session| session.owns_templates(&ctx.physical_interface_declaration_templates));
    let cached = if let Some(session) = session.as_ref() {
        session.template_lookup(declaration)
    } else {
        ctx.physical_interface_declaration_templates
            .lock()
            .ok()
            .and_then(|cache| cache.get(declaration).cloned())
    };
    if let Some(template) = cached {
        crate::program::record_program_counter(|c| {
            c.interface_template_hit_count += 1;
        });
        return Some(template);
    }

    crate::program::record_program_counter(|c| c.interface_template_build_attempt_count += 1);
    if interface.body.members.len() != interface.body.member_fragments.len() {
        return None;
    }

    let mut overload_indices = std::collections::HashMap::<&str, u32>::new();
    let mut group_indices = std::collections::HashMap::<&str, u32>::new();
    let mut group_members = Vec::<Vec<StableInterfaceMemberDeclarationId>>::new();
    let mut members = Vec::with_capacity(interface.body.members.len());
    for (member, fragment) in interface
        .body
        .members
        .iter()
        .zip(interface.body.member_fragments.iter())
    {
        let declaration_kind = if matches!(member.ty, ParsedType::Function(_)) {
            InterfaceMemberDeclarationKind::Method
        } else {
            InterfaceMemberDeclarationKind::Property
        };
        let overload_index = if declaration_kind == InterfaceMemberDeclarationKind::Method {
            let next = overload_indices.entry(member.name.as_str()).or_default();
            let current = *next;
            *next = next.checked_add(1)?;
            current
        } else {
            0
        };
        let member_declaration = StableInterfaceMemberDeclarationId {
            containing_interface: declaration.clone(),
            canonical_file: canonical_declaration_file_name(&fragment.file_name),
            declaration_start: u32::try_from(member.name_span?.start).ok()?,
            declaration_kind,
            declared_name: Arc::from(member.name.as_str()),
            overload_index,
        };
        let (overload_group, overload_position) =
            if declaration_kind == InterfaceMemberDeclarationKind::Method {
                let group = match group_indices.get(member.name.as_str()).copied() {
                    Some(group) => group,
                    None => {
                        let group = u32::try_from(group_members.len()).ok()?;
                        group_indices.insert(member.name.as_str(), group);
                        group_members.push(Vec::new());
                        group
                    }
                };
                let group_members = group_members.get_mut(group as usize)?;
                let position = u32::try_from(group_members.len()).ok()?;
                group_members.push(member_declaration.clone());
                (Some(group), position)
            } else {
                (None, 0)
            };
        members.push(InterfaceMemberDeclarationTemplate {
            declaration: member_declaration,
            overload_group,
            overload_position,
        });
    }

    let method_groups = group_members
        .into_iter()
        .map(|ordered_members| InterfaceMethodOverloadGroupTemplate {
            ordered_members: Arc::from(ordered_members),
        })
        .collect::<Vec<_>>();
    let retained_bytes = std::mem::size_of::<InterfaceDeclarationTemplate>()
        .saturating_add(members.len() * std::mem::size_of::<InterfaceMemberDeclarationTemplate>())
        .saturating_add(
            method_groups
                .iter()
                .map(|group| {
                    std::mem::size_of::<InterfaceMethodOverloadGroupTemplate>()
                        + group.ordered_members.len()
                            * std::mem::size_of::<StableInterfaceMemberDeclarationId>()
                })
                .sum::<usize>(),
        ) as u64;
    let template = Arc::new(InterfaceDeclarationTemplate {
        members: Arc::from(members),
        method_groups: Arc::from(method_groups),
    });
    if let Some(session) = session {
        return Some(session.template_intern(declaration.clone(), template, retained_bytes));
    }
    let Ok(mut cache) = ctx.physical_interface_declaration_templates.lock() else {
        return Some(template);
    };
    let template = cache
        .entry(declaration.clone())
        .or_insert_with(|| {
            crate::program::record_program_counter(|c| {
                c.interface_template_insert_count += 1;
                c.interface_template_retained_bytes += retained_bytes;
            });
            template
        })
        .clone();
    Some(template)
}

pub(crate) fn interface_member_instantiation_key(
    member: &StableInterfaceMemberDeclarationId,
    interface: &InterfaceInstantiationKey,
) -> InterfaceMemberInstantiationKey {
    InterfaceMemberInstantiationKey {
        member: member.clone(),
        substitution: interface.substitution,
        environment: interface.environment,
    }
}

pub(crate) fn lookup_physical_interface_method(
    ctx: &CheckerContext,
    key: &InterfaceMemberInstantiationKey,
) -> Option<FunctionType> {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_methods(&ctx.physical_interface_method_instantiations))
    {
        return session.method_lookup(key);
    }
    ctx.physical_interface_method_instantiations
        .lock()
        .ok()?
        .get(key)
        .cloned()
}

pub(crate) fn intern_physical_interface_method(
    ctx: &CheckerContext,
    key: InterfaceMemberInstantiationKey,
    function: FunctionType,
) -> FunctionType {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_methods(&ctx.physical_interface_method_instantiations))
    {
        let key_bytes = std::mem::size_of::<InterfaceMemberInstantiationKey>() as u64;
        let value_bytes = interface_function_value_shallow_bytes(&function) as u64;
        return session.method_intern(key, function, key_bytes, value_bytes);
    }
    let Ok(mut cache) = ctx.physical_interface_method_instantiations.lock() else {
        return function;
    };
    if let Some(existing) = cache.get(&key) {
        return existing.clone();
    }
    let key_bytes = std::mem::size_of::<InterfaceMemberInstantiationKey>();
    let value_bytes = interface_function_value_shallow_bytes(&function);
    cache.insert(key, function.clone());
    crate::program::record_program_counter(|c| {
        c.interface_method_cache_insert_count += 1;
        c.interface_method_cache_key_bytes += key_bytes as u64;
        c.interface_method_cache_value_shallow_bytes += value_bytes as u64;
    });
    function
}

pub(crate) fn interface_overload_instantiation_key(
    declaration: &StableInterfaceDeclarationId,
    group: &InterfaceMethodOverloadGroupTemplate,
    prefix_len: u32,
    interface: &InterfaceInstantiationKey,
) -> InterfaceOverloadInstantiationKey {
    InterfaceOverloadInstantiationKey {
        containing_interface: declaration.clone(),
        ordered_members: group.ordered_members.clone(),
        prefix_len,
        substitution: interface.substitution,
        environment: interface.environment,
    }
}

pub(crate) fn lookup_physical_interface_overload(
    ctx: &CheckerContext,
    key: &InterfaceOverloadInstantiationKey,
) -> Option<FunctionType> {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_overloads(&ctx.physical_interface_overload_instantiations))
    {
        return session.overload_lookup(key);
    }
    ctx.physical_interface_overload_instantiations
        .lock()
        .ok()?
        .get(key)
        .cloned()
}

pub(crate) fn intern_physical_interface_overload(
    ctx: &CheckerContext,
    key: InterfaceOverloadInstantiationKey,
    function: FunctionType,
) -> FunctionType {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_overloads(&ctx.physical_interface_overload_instantiations))
    {
        let key_bytes = std::mem::size_of::<InterfaceOverloadInstantiationKey>() as u64;
        let value_bytes = interface_function_value_shallow_bytes(&function) as u64;
        return session.overload_intern(key, function, key_bytes, value_bytes);
    }
    let Ok(mut cache) = ctx.physical_interface_overload_instantiations.lock() else {
        return function;
    };
    if let Some(existing) = cache.get(&key) {
        return existing.clone();
    }
    let key_bytes = std::mem::size_of::<InterfaceOverloadInstantiationKey>();
    let value_bytes = interface_function_value_shallow_bytes(&function);
    cache.insert(key, function.clone());
    crate::program::record_program_counter(|c| {
        c.interface_overload_cache_insert_count += 1;
        c.interface_overload_cache_key_bytes += key_bytes as u64;
        c.interface_overload_cache_value_shallow_bytes += value_bytes as u64;
    });
    function
}

fn interface_function_value_shallow_bytes(function: &FunctionType) -> usize {
    std::mem::size_of::<FunctionType>()
        + std::mem::size_of::<surge_ts_types::FunctionTypePayload>()
        + function.parameters().len() * std::mem::size_of::<Type>()
}

pub(crate) fn canonical_physical_interface_key(
    interface: &crate::symbols::InterfaceInfo,
    substitution: &TypeParameterSubstitution,
    ctx: &CheckerContext,
) -> Result<InterfaceInstantiationKey, InterfaceCacheSkipReason> {
    let declaration = stable_interface_declaration_id(interface)?;
    canonical_physical_interface_key_with_declaration(interface, substitution, ctx, declaration)
}

pub(crate) fn stable_interface_declaration_id(
    interface: &crate::symbols::InterfaceInfo,
) -> Result<StableInterfaceDeclarationId, InterfaceCacheSkipReason> {
    let declaration_start = interface
        .name_span
        .map_or(Ok(0), |span| u32::try_from(span.start))
        .map_err(|_| InterfaceCacheSkipReason::UnstableDeclaration)?;
    let mut merged_fragments = Vec::with_capacity(interface.body.declaration_fragments.len());
    for fragment in &interface.body.declaration_fragments {
        merged_fragments.push(StableInterfaceDeclarationFragmentId {
            canonical_file: canonical_declaration_file_name(&fragment.file_name),
            declaration_start: u32::try_from(fragment.declaration_start)
                .map_err(|_| InterfaceCacheSkipReason::UnstableDeclaration)?,
        });
    }
    Ok(StableInterfaceDeclarationId {
        canonical_file: canonical_declaration_file_name(&interface.file_name),
        declaration_start,
        declaration_name: Arc::from(
            interface
                .declared_name
                .as_deref()
                .unwrap_or(interface.name.as_str()),
        ),
        merged_fragments: Arc::from(merged_fragments),
    })
}

fn canonical_physical_interface_key_with_declaration(
    interface: &crate::symbols::InterfaceInfo,
    substitution: &TypeParameterSubstitution,
    ctx: &CheckerContext,
    declaration: StableInterfaceDeclarationId,
) -> Result<InterfaceInstantiationKey, InterfaceCacheSkipReason> {
    let mut arguments = Vec::with_capacity(interface.body.type_parameters.len());
    let mut budget = 128usize;
    for parameter in &interface.body.type_parameters {
        if substitution.is_placeholder(&parameter.name) {
            return Err(InterfaceCacheSkipReason::UnresolvedTypeArgument);
        }
        let Some(argument) = substitution.get(&parameter.name) else {
            return Err(InterfaceCacheSkipReason::UnresolvedTypeArgument);
        };
        // Display-inclusive identity: the deep display fingerprint keeps
        // structurally-equal-but-differently-rendered arguments apart, so a
        // cached instantiation never substitutes another context's rendering
        // (the canonical-store display-substitution class).
        arguments.push(CanonicalTypeIdentity::DisplayTagged(
            Box::new(canonical_type_identity(argument, 0, &mut budget)?),
            crate::speculative::display_type_fingerprint(argument),
        ));
    }

    let substitution = ctx
        .substitution_store
        .intern(declaration.clone(), arguments);
    Ok(InterfaceInstantiationKey {
        declaration,
        substitution,
        environment: InterfaceEnvironmentIdentity {
            no_lib: ctx.options.no_lib,
            skip_lib_check: ctx.options.skip_lib_check,
        },
    })
}

fn canonical_type_identity(
    ty: &Type,
    depth: usize,
    budget: &mut usize,
) -> Result<CanonicalTypeIdentity, InterfaceCacheSkipReason> {
    if depth >= 32 || *budget == 0 {
        return Err(InterfaceCacheSkipReason::UnsupportedTypeArgument);
    }
    *budget -= 1;

    let primitive = match ty {
        Type::String => Some(CanonicalTypeIdentity::String),
        Type::Number => Some(CanonicalTypeIdentity::Number),
        Type::Boolean => Some(CanonicalTypeIdentity::Boolean),
        Type::BigInt => Some(CanonicalTypeIdentity::BigInt),
        Type::Symbol => Some(CanonicalTypeIdentity::Symbol),
        Type::Undefined => Some(CanonicalTypeIdentity::Undefined),
        Type::Void => Some(CanonicalTypeIdentity::Void),
        Type::Any => Some(CanonicalTypeIdentity::Any),
        Type::Never => Some(CanonicalTypeIdentity::Never),
        Type::StringLiteral(value) => Some(CanonicalTypeIdentity::StringLiteral(Arc::from(
            value.as_str(),
        ))),
        Type::NumberLiteral(value) => Some(CanonicalTypeIdentity::NumberLiteral(Arc::from(
            value.value.as_str(),
        ))),
        Type::BooleanLiteral(value) => Some(CanonicalTypeIdentity::BooleanLiteral(*value)),
        _ => None,
    };
    if let Some(identity) = primitive {
        return Ok(identity);
    }

    match ty {
        Type::Array(element) => Ok(CanonicalTypeIdentity::Array(Box::new(
            canonical_type_identity(element, depth + 1, budget)?,
        ))),
        Type::Tuple(elements) => {
            let mut identities = Vec::with_capacity(elements.len());
            for element in elements {
                identities.push(canonical_type_identity(element, depth + 1, budget)?);
            }
            Ok(CanonicalTypeIdentity::Tuple(Arc::from(identities)))
        }
        Type::Reference(reference) => {
            let mut arguments = Vec::with_capacity(reference.arguments.len());
            for argument in reference.arguments.iter() {
                arguments.push(canonical_type_identity(argument, depth + 1, budget)?);
            }
            Ok(CanonicalTypeIdentity::Reference {
                declaration: reference.id.clone(),
                arguments: Arc::from(arguments),
            })
        }
        Type::Object(object) => object
            .alias_id
            .clone()
            .map(CanonicalTypeIdentity::NamedObject)
            .ok_or(InterfaceCacheSkipReason::UnsupportedTypeArgument),
        Type::Unknown | Type::GenuineUnknown => {
            Err(InterfaceCacheSkipReason::UnresolvedTypeArgument)
        }
        Type::Function(_) | Type::Union(_) => {
            Err(InterfaceCacheSkipReason::UnsupportedTypeArgument)
        }
        _ => unreachable!("primitive canonical identities returned above"),
    }
}

pub(crate) fn lookup_physical_interface_instantiation(
    ctx: &CheckerContext,
    key: &InterfaceInstantiationKey,
) -> Option<Arc<Type>> {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_physical(&ctx.physical_interface_instantiations))
    {
        return session.physical_lookup(key);
    }
    let cache = ctx.physical_interface_instantiations.lock().ok()?;
    cache.get(key).cloned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterfaceCacheValueRejection {
    Unknown,
    ResolutionContext,
    TraversalLimit,
}

pub(crate) fn validate_physical_interface_cache_value(
    resolved: &Type,
) -> Result<(), InterfaceCacheValueRejection> {
    fn visit(
        ty: &Type,
        depth: usize,
        budget: &mut usize,
    ) -> Result<(), InterfaceCacheValueRejection> {
        if depth >= 64 || *budget == 0 {
            return Err(InterfaceCacheValueRejection::TraversalLimit);
        }
        *budget -= 1;
        match ty {
            Type::Function(function) => {
                for parameter in function.parameters() {
                    visit(parameter, depth + 1, budget)?;
                }
                visit(function.return_type(), depth + 1, budget)
            }
            Type::Object(object) => {
                for property in object.properties.values() {
                    visit(&property.ty, depth + 1, budget)?;
                }
                if let Some(indexed) = object.string_index_type.as_deref() {
                    visit(indexed, depth + 1, budget)?;
                }
                for signature in [object.construct_signature(), object.call_signature()]
                    .into_iter()
                    .flatten()
                {
                    for parameter in signature.parameters() {
                        visit(parameter, depth + 1, budget)?;
                    }
                    visit(signature.return_type(), depth + 1, budget)?;
                }
                Ok(())
            }
            Type::Array(element) => visit(element, depth + 1, budget),
            Type::Tuple(elements) => {
                for element in elements {
                    visit(element, depth + 1, budget)?;
                }
                Ok(())
            }
            Type::Union(union) => {
                for member in union.types() {
                    visit(member, depth + 1, budget)?;
                }
                Ok(())
            }
            Type::Reference(reference) => {
                if reference.retains_resolution_context() {
                    return Err(InterfaceCacheValueRejection::ResolutionContext);
                }
                for argument in reference.arguments.iter() {
                    visit(argument, depth + 1, budget)?;
                }
                Ok(())
            }
            Type::Unknown => Err(InterfaceCacheValueRejection::Unknown),
            _ => Ok(()),
        }
    }

    visit(resolved, 0, &mut 512)
}

pub(crate) fn physical_interface_method_has_contextual_typing_dependency(resolved: &Type) -> bool {
    fn contains_callable(ty: &Type, depth: usize, budget: &mut usize) -> bool {
        if depth >= 32 || *budget == 0 {
            return true;
        }
        *budget -= 1;
        match ty {
            Type::Function(_) => true,
            Type::Object(object) => {
                object.call_signature().is_some()
                    || object.construct_signature().is_some()
                    || object
                        .properties
                        .values()
                        .any(|property| contains_callable(&property.ty, depth + 1, budget))
            }
            Type::Array(element) => contains_callable(element, depth + 1, budget),
            Type::Tuple(elements) => elements
                .iter()
                .any(|element| contains_callable(element, depth + 1, budget)),
            Type::Union(union) => union
                .types()
                .iter()
                .any(|member| contains_callable(member, depth + 1, budget)),
            _ => false,
        }
    }

    let Type::Function(function) = resolved else {
        return false;
    };
    let mut budget = 128;
    function
        .parameters()
        .iter()
        .any(|parameter| contains_callable(parameter, 0, &mut budget))
}

pub(crate) fn intern_physical_interface_instantiation(
    ctx: &CheckerContext,
    key: InterfaceInstantiationKey,
    resolved: Type,
) -> Arc<Type> {
    if let Some(session) = crate::speculative::active_check_session()
        .filter(|session| session.owns_physical(&ctx.physical_interface_instantiations))
    {
        return session.physical_intern(key, resolved);
    }
    let Ok(mut cache) = ctx.physical_interface_instantiations.lock() else {
        return Arc::new(resolved);
    };
    if let Some(existing) = cache.get(&key) {
        crate::program::record_program_counter(|c| {
            c.physical_interface_cache_racing_insert_count += 1
        });
        return existing.clone();
    }

    let key_bytes = interface_key_shallow_bytes(&key);
    let value_bytes = interface_value_shallow_bytes(&resolved);
    let resolved = Arc::new(resolved);
    cache.insert(key, resolved.clone());
    crate::program::record_program_counter(|c| {
        c.physical_interface_cache_insert_count += 1;
        c.physical_interface_cache_key_bytes += key_bytes;
        c.physical_interface_cache_value_shallow_bytes += value_bytes;
    });
    resolved
}

fn interface_key_shallow_bytes(key: &InterfaceInstantiationKey) -> u64 {
    let mut bytes = std::mem::size_of::<InterfaceInstantiationKey>()
        + key.declaration.canonical_file.len()
        + key.declaration.declaration_name.len();
    bytes += key
        .declaration
        .merged_fragments
        .iter()
        .map(|fragment| {
            std::mem::size_of::<StableInterfaceDeclarationFragmentId>()
                + fragment.canonical_file.len()
        })
        .sum::<usize>();
    bytes as u64
}

fn interface_value_shallow_bytes(resolved: &Type) -> u64 {
    let mut bytes = std::mem::size_of::<Type>();
    if let Type::Object(object) = resolved {
        bytes += std::mem::size_of::<surge_ts_types::ObjectType>();
        bytes += object.properties.capacity()
            * (std::mem::size_of::<String>()
                + std::mem::size_of::<surge_ts_types::ObjectProperty>());
    }
    bytes as u64
}

/// Builds a lazy/nominal [`Type::Reference`] over a shared structural expansion.
/// `id` is the nominal identity (`file\u{0}name`), `display` the diagnostic form
/// (e.g. `Box<string>`), and `arguments` the resolved type arguments.
#[allow(dead_code)]
pub(crate) fn make_type_reference(
    id: impl Into<Arc<str>>,
    display: impl Into<Arc<str>>,
    arguments: Vec<Type>,
    resolved: Arc<Type>,
) -> Type {
    Type::Reference(TypeReference::new(
        id,
        display,
        arguments,
        Arc::new(InternedInstantiation { resolved }),
    ))
}

#[cfg(test)]
mod physical_interface_cache_tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use surge_ts_syntax::{
        ParsedFunctionType, ParsedInterfaceMember, ParsedTypeParameter, TextSpan,
    };
    use surge_ts_types::{
        FunctionType, ObjectProperty, ObjectType, PropertyMap, ResolveReference, Type,
        TypeReference, union_type,
    };

    use super::*;
    use crate::context::{CheckerContext, CheckerOptions, FileKind};
    use crate::symbols::{InterfaceInfo, merge_interface_infos};

    const LIB_DOM: &str = "/typescript/lib/lib.dom.d.ts";

    fn interface(name: &str, start: usize, type_parameters: &[&str]) -> InterfaceInfo {
        InterfaceInfo::new(
            name.to_string(),
            LIB_DOM.to_string(),
            Some(TextSpan {
                start,
                end: start + name.len(),
            }),
            type_parameters
                .iter()
                .map(|name| ParsedTypeParameter {
                    name: (*name).to_string(),
                    name_span: None,
                    constraint: None,
                    default_type: None,
                    span: None,
                })
                .collect(),
            Vec::new(),
            Vec::new(),
            None,
            None,
            Vec::new(),
            None,
        )
    }

    fn context(options: CheckerOptions) -> CheckerContext {
        CheckerContext::new(
            LIB_DOM.to_string(),
            options,
            surge_ts_types::fx::FxHashMap::from_iter([(
                LIB_DOM.to_string(),
                FileKind::PhysicalDefaultLib,
            )]),
        )
    }

    fn interface_with_methods(
        name: &str,
        start: usize,
        type_parameters: &[&str],
        methods: &[(&str, usize)],
    ) -> InterfaceInfo {
        let mut interface = interface(name, start, type_parameters);
        let body = Arc::make_mut(&mut interface.body);
        body.members = methods
            .iter()
            .map(|(name, start)| ParsedInterfaceMember {
                name: (*name).to_string(),
                name_span: Some(TextSpan {
                    start: *start,
                    end: *start + name.len(),
                }),
                optional: false,
                is_abstract: false,
                ty: ParsedType::Function(std::sync::Arc::new(ParsedFunctionType {
                    parameters: Vec::new(),
                    return_type: Box::new(ParsedType::String),
                    type_parameters: Vec::new(),
                })),
            })
            .collect();
        body.member_fragments = vec![body.declaration_fragments[0].clone(); body.members.len()];
        interface
    }

    #[test]
    fn physical_lib_interface_cache_basic() {
        let interface = interface("Body", 100, &[]);
        let ctx = context(CheckerOptions::default());
        let key =
            canonical_physical_interface_key(&interface, &TypeParameterSubstitution::new(), &ctx)
                .unwrap();

        assert_eq!(ctx.substitution_store.stats().stored_arguments, 0);
        assert_eq!(key.declaration.declaration_start, 100);
        assert_eq!(&*key.declaration.declaration_name, "Body");
    }

    #[test]
    fn physical_lib_interface_cache_generic_same_args_many_consumers() {
        let interface = interface("Iterator", 200, &["T"]);
        let ctx = context(CheckerOptions::default());
        let mut substitution = TypeParameterSubstitution::new();
        substitution.insert("T".to_string(), Type::String);
        let key = canonical_physical_interface_key(&interface, &substitution, &ctx).unwrap();

        let first = intern_physical_interface_instantiation(&ctx, key.clone(), Type::String);
        let second = intern_physical_interface_instantiation(&ctx, key.clone(), Type::Number);
        let lookup = lookup_physical_interface_instantiation(&ctx, &key).unwrap();

        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&first, &lookup));
        assert_eq!(*lookup, Type::String);
    }

    #[test]
    fn physical_lib_interface_cache_different_args() {
        let interface = interface("Iterator", 200, &["T"]);
        let ctx = context(CheckerOptions::default());
        let mut strings = TypeParameterSubstitution::new();
        strings.insert("T".to_string(), Type::String);
        let mut numbers = TypeParameterSubstitution::new();
        numbers.insert("T".to_string(), Type::Number);

        let string_key = canonical_physical_interface_key(&interface, &strings, &ctx).unwrap();
        let number_key = canonical_physical_interface_key(&interface, &numbers, &ctx).unwrap();

        assert_ne!(string_key, number_key);
    }

    #[test]
    fn physical_lib_interface_cache_degraded_not_cacheable() {
        let interface = interface("Iterator", 200, &["T"]);
        let ctx = context(CheckerOptions::default());
        let mut unknown = TypeParameterSubstitution::new();
        unknown.insert("T".to_string(), Type::Unknown);
        let mut placeholder = TypeParameterSubstitution::new();
        placeholder.insert_placeholder("T".to_string(), Type::String);

        assert_eq!(
            canonical_physical_interface_key(&interface, &unknown, &ctx),
            Err(InterfaceCacheSkipReason::UnresolvedTypeArgument)
        );
        assert_eq!(
            canonical_physical_interface_key(&interface, &placeholder, &ctx),
            Err(InterfaceCacheSkipReason::UnresolvedTypeArgument)
        );
    }

    #[test]
    fn physical_lib_interface_cache_interface_merge_identity() {
        let original = interface("Element", 300, &[]);
        let augmentation = interface("Element", 900, &[]);
        let merged = merge_interface_infos(&original, &augmentation);

        let original_id = stable_interface_declaration_id(&original).unwrap();
        let merged_id = stable_interface_declaration_id(&merged).unwrap();

        assert_ne!(original_id, merged_id);
        assert_eq!(merged_id.merged_fragments.len(), 2);
        assert_eq!(merged_id.merged_fragments[0].declaration_start, 300);
        assert_eq!(merged_id.merged_fragments[1].declaration_start, 900);
    }

    #[test]
    fn physical_lib_interface_cache_environment_identity() {
        let interface = interface("Element", 300, &[]);
        let default_ctx = context(CheckerOptions::default());
        let mut skip_lib_options = CheckerOptions::default();
        skip_lib_options.skip_lib_check = true;
        let skip_lib_ctx = context(skip_lib_options);

        let default_key = canonical_physical_interface_key(
            &interface,
            &TypeParameterSubstitution::new(),
            &default_ctx,
        )
        .unwrap();
        let skip_lib_key = canonical_physical_interface_key(
            &interface,
            &TypeParameterSubstitution::new(),
            &skip_lib_ctx,
        )
        .unwrap();

        assert_ne!(default_key, skip_lib_key);
    }

    #[test]
    fn physical_lib_interface_cache_rejects_unknown_and_context() {
        struct ContextualResolver;

        impl ResolveReference for ContextualResolver {
            fn resolve(&self) -> Type {
                Type::String
            }

            fn retains_resolution_context(&self) -> bool {
                true
            }
        }

        let contextual = Type::Reference(TypeReference::new(
            "lib.dom.d.ts\0Contextual",
            "Contextual",
            Vec::<Type>::new(),
            Arc::new(ContextualResolver),
        ));

        assert_eq!(
            validate_physical_interface_cache_value(&Type::Unknown),
            Err(InterfaceCacheValueRejection::Unknown)
        );
        assert_eq!(
            validate_physical_interface_cache_value(&contextual),
            Err(InterfaceCacheValueRejection::ResolutionContext)
        );
        assert!(validate_physical_interface_cache_value(&Type::GenuineUnknown).is_ok());
        let contextual_method = Type::Function(FunctionType::new(
            vec![Type::Function(FunctionType::new(
                vec![Type::String],
                Type::Void,
                false,
                1,
            ))],
            Type::Void,
            false,
            1,
        ));
        assert!(physical_interface_method_has_contextual_typing_dependency(
            &contextual_method
        ));
    }

    #[test]
    fn physical_lib_interface_cache_preserves_overloads_and_signatures() {
        let interface = interface("Callable", 1_000, &[]);
        let ctx = context(CheckerOptions::default());
        let key =
            canonical_physical_interface_key(&interface, &TypeParameterSubstitution::new(), &ctx)
                .unwrap();
        let first_overload = Type::Function(FunctionType::new(
            vec![Type::String],
            Type::Number,
            false,
            1,
        ));
        let second_overload = Type::Function(FunctionType::new(
            vec![Type::Number],
            Type::String,
            false,
            1,
        ));
        let mut properties = PropertyMap::default();
        properties.insert(
            "method".into(),
            ObjectProperty::required(union_type(vec![first_overload, second_overload])),
        );
        let object = ObjectType::new(properties, Some(Type::String))
            .with_call_signature(FunctionType::new(Vec::new(), Type::Boolean, false, 0))
            .with_construct_signature(FunctionType::new(Vec::new(), Type::Any, false, 0));

        let cached =
            intern_physical_interface_instantiation(&ctx, key.clone(), Type::Object(object));
        let lookup = lookup_physical_interface_instantiation(&ctx, &key).unwrap();
        assert!(Arc::ptr_eq(&cached, &lookup));
        let Type::Object(object) = &*lookup else {
            panic!("cached interface must remain an object");
        };
        let Type::Union(overloads) = &object.get_property("method").unwrap().ty else {
            panic!("method overload array must remain ordered");
        };
        assert_eq!(overloads.types()[0].name(), "(string) => number");
        assert_eq!(overloads.types()[1].name(), "(number) => string");
        assert_eq!(
            object.call_signature().unwrap().return_type(),
            &Type::Boolean
        );
        assert_eq!(
            object.construct_signature().unwrap().return_type(),
            &Type::Any
        );
        assert_eq!(object.string_index_type.as_deref(), Some(&Type::String));
    }

    #[test]
    fn physical_lib_interface_cache_recursive_reference_is_not_peeled() {
        struct RecursiveResolver;

        impl ResolveReference for RecursiveResolver {
            fn resolve(&self) -> Type {
                panic!("cache identity and eligibility must not peel references")
            }
        }

        let recursive = Type::Reference(TypeReference::new(
            "lib.es2015.iterable.d.ts\0IterableIterator",
            "IterableIterator<string>",
            vec![Type::String],
            Arc::new(RecursiveResolver),
        ));
        assert!(validate_physical_interface_cache_value(&recursive).is_ok());

        let interface = interface("IterableIterator", 1_100, &["T"]);
        let ctx = context(CheckerOptions::default());
        let mut substitution = TypeParameterSubstitution::new();
        substitution.insert("T".to_string(), recursive);
        let _key = canonical_physical_interface_key(&interface, &substitution, &ctx).unwrap();

        assert_eq!(ctx.substitution_store.stats().stored_arguments, 1);
    }

    #[test]
    fn physical_lib_method_cache_uses_stable_member_and_substitution_identity() {
        let interface = interface_with_methods("Iterator", 2_000, &["T"], &[("next", 2_010)]);
        let ctx = context(CheckerOptions::default());
        let mut substitution = TypeParameterSubstitution::new();
        substitution.insert("T".to_string(), Type::String);
        let interface_key =
            canonical_physical_interface_key(&interface, &substitution, &ctx).unwrap();
        let template =
            physical_interface_declaration_template(&ctx, &interface, &interface_key.declaration)
                .unwrap();
        let key =
            interface_member_instantiation_key(&template.members[0].declaration, &interface_key);
        let first = intern_physical_interface_method(
            &ctx,
            key.clone(),
            FunctionType::new(Vec::new(), Type::String, false, 0),
        );
        let second = intern_physical_interface_method(
            &ctx,
            key.clone(),
            FunctionType::new(Vec::new(), Type::Number, false, 0),
        );

        assert!(std::ptr::eq(first.payload(), second.payload()));
        assert!(std::ptr::eq(
            first.payload(),
            lookup_physical_interface_method(&ctx, &key)
                .unwrap()
                .payload()
        ));

        let mut number_substitution = TypeParameterSubstitution::new();
        number_substitution.insert("T".to_string(), Type::Number);
        let number_interface_key =
            canonical_physical_interface_key(&interface, &number_substitution, &ctx).unwrap();
        let number_key = interface_member_instantiation_key(
            &template.members[0].declaration,
            &number_interface_key,
        );
        assert_ne!(key, number_key);
        assert!(lookup_physical_interface_method(&ctx, &number_key).is_none());
    }

    #[test]
    fn physical_lib_overload_cache_preserves_declaration_order_and_duplicates() {
        let interface = interface_with_methods(
            "Headers",
            3_000,
            &[],
            &[("append", 3_010), ("append", 3_020), ("append", 3_030)],
        );
        let ctx = context(CheckerOptions::default());
        let interface_key =
            canonical_physical_interface_key(&interface, &TypeParameterSubstitution::new(), &ctx)
                .unwrap();
        let template =
            physical_interface_declaration_template(&ctx, &interface, &interface_key.declaration)
                .unwrap();
        let group = &template.method_groups[0];

        assert_eq!(group.ordered_members.len(), 3);
        assert_eq!(group.ordered_members[0].declaration_start, 3_010);
        assert_eq!(group.ordered_members[1].declaration_start, 3_020);
        assert_eq!(group.ordered_members[2].declaration_start, 3_030);
        assert_eq!(group.ordered_members[0].overload_index, 0);
        assert_eq!(group.ordered_members[1].overload_index, 1);
        assert_eq!(group.ordered_members[2].overload_index, 2);

        let key = interface_overload_instantiation_key(
            &interface_key.declaration,
            group,
            3,
            &interface_key,
        );
        let first = intern_physical_interface_overload(
            &ctx,
            key.clone(),
            FunctionType::new(vec![Type::String], Type::String, false, 1),
        );
        let second = lookup_physical_interface_overload(&ctx, &key).unwrap();
        assert!(std::ptr::eq(first.payload(), second.payload()));
    }
}
