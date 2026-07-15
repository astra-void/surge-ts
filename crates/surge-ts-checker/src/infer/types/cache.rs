//! Named-type resolution memoization and declaration resolution keys.

use super::*;

use std::path::Path;
use std::sync::Arc;

use surge_ts_types::{ResolveReference, Type, TypeReference};

use crate::context::{
    CheckerContext, DeclarationNamespace, DeclarationResolutionKey, DeclarationResolutionState,
    GenericInstantiationCacheEntry, InstantiationCacheEntry,
};
use crate::paths::canonicalize_if_exists_string;
use crate::symbols::TypeDeclarationInfo;

pub(crate) fn type_declaration_resolution_key(
    declaration: &TypeDeclarationInfo,
) -> DeclarationResolutionKey {
    match declaration {
        TypeDeclarationInfo::Alias(alias) => DeclarationResolutionKey {
            file_name: canonical_declaration_file_name(&alias.file_name),
            name: alias.name.clone(),
            namespace: DeclarationNamespace::Type,
        },
        TypeDeclarationInfo::Interface(interface) => DeclarationResolutionKey {
            file_name: canonical_declaration_file_name(&interface.file_name),
            name: interface.name.clone(),
            namespace: DeclarationNamespace::Type,
        },
    }
}

pub(crate) fn declaration_resolution_key(file_name: &str, name: &str) -> DeclarationResolutionKey {
    DeclarationResolutionKey {
        file_name: canonical_declaration_file_name(file_name),
        name: name.to_string(),
        namespace: DeclarationNamespace::Type,
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
fn generic_instantiation_bucket_cap() -> usize {
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
    let resolved = ctx
        .program_resolved_generic_types
        .lock()
        .ok()
        .and_then(|cache| {
            cache.get(key)?.iter().find_map(|entry| {
                (entry.arguments == arguments).then(|| ResolvedType {
                    ty: entry.ty.clone(),
                    had_error: entry.had_error,
                })
            })
        });
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

pub(crate) fn canonical_declaration_file_name(file_name: &str) -> String {
    canonicalize_if_exists_string(Path::new(file_name))
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
    snapshot: Arc<CheckerContext>,
    /// The type-declaration scope installed where this reference was created.
    /// The shared snapshot is captured once per context — often before any
    /// module scope is installed — so a peel through it alone cannot see the
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
    snapshot: Arc<CheckerContext>,
    creation_scope: Option<Arc<crate::symbols::TypeDeclarationScope>>,
    key: DeclarationResolutionKey,
    display: Arc<str>,
    annotation: surge_ts_syntax::ParsedType,
    signature_component: Option<LazySignatureComponent>,
    signature_environment: Option<LazySignatureEnvironment>,
    memo: std::sync::OnceLock<std::sync::Weak<Type>>,
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
        if let Some(entry) = lookup_instantiation(&self.snapshot, &self.key, &[]) {
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
            &self.snapshot.file_name,
            &self.display,
            0,
        );
        let mut ctx = Box::new((*self.snapshot).clone());
        ctx.set_file_name(self.key.file_name.clone());
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
        let resolved = Arc::new(match resolved.ty {
            Type::Reference(reference) => reference.resolve().peeled(),
            ty => ty,
        });
        if had_error || resolved.is_unknown() {
            crate::program::record_program_counter(|c| {
                c.lazy_reference_degraded_expansion_count += 1;
                if self.signature_component.is_some() {
                    c.degraded_signature_expansion_count += 1;
                }
            });
            if self.signature_component.is_some() {
                crate::program::record_degraded_signature_expansion(&self.key);
            }
            return resolved;
        }
        let resolved = intern_instantiation(&self.snapshot, &self.key, &[], (*resolved).clone());
        let _ = self.memo.set(Arc::downgrade(&resolved));
        crate::program::record_lazy_reference_expansion(
            &self.key,
            &self.snapshot.file_name,
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

pub(crate) fn make_lazy_declaration_annotation_reference(
    ctx: &mut CheckerContext,
    declaration_name: &str,
    declaration_start: usize,
    annotation: surge_ts_syntax::ParsedType,
) -> Type {
    crate::program::record_program_counter(|c| c.lazy_annotation_reference_create_count += 1);
    let display: Arc<str> = Arc::from(parsed_annotation_display(&annotation));
    let key = DeclarationResolutionKey {
        file_name: canonical_declaration_file_name(&ctx.file_name),
        name: format!("value {declaration_name}@{declaration_start}"),
        namespace: DeclarationNamespace::Type,
    };
    crate::program::record_lazy_reference_created(&key);
    let id = format!(
        "{}\u{0}value-annotation\u{0}{declaration_name}\u{0}{declaration_start}",
        key.file_name
    );
    let snapshot = ctx.lazy_resolution_snapshot();
    let creation_scope = ctx.type_declaration_scope.clone();
    Type::Reference(TypeReference::new(
        id,
        display.clone(),
        Vec::new(),
        Arc::new(LazyDeclarationAnnotation {
            snapshot,
            creation_scope,
            key,
            display,
            annotation,
            signature_component: None,
            signature_environment: None,
            memo: std::sync::OnceLock::new(),
        }),
    ))
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
        name: format!("signature {declaration_name}@{declaration_start}:{component_identity}"),
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
    let snapshot = ctx.lazy_resolution_snapshot();
    let creation_scope = ctx.type_declaration_scope.clone();
    Type::Reference(TypeReference::new(
        id,
        display.clone(),
        Vec::new(),
        Arc::new(LazyDeclarationAnnotation {
            snapshot,
            creation_scope,
            key,
            display,
            annotation,
            signature_component: Some(component),
            signature_environment,
            memo: std::sync::OnceLock::new(),
        }),
    ))
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
                    call.as_ref().clone(),
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

    fn resolve_arc(&self) -> Arc<Type> {
        crate::program::record_lazy_reference_peel_start(&self.decl_key);
        if let Some(memoized) = self.memo.get().and_then(std::sync::Weak::upgrade) {
            crate::program::record_program_counter(|c| c.lazy_reference_memo_hit_count += 1);
            return memoized;
        }
        // A peel of the same instantiation elsewhere may have already interned it.
        if let Some(entry) =
            lookup_instantiation(&self.snapshot, &self.decl_key, &self.resolved_arguments)
        {
            crate::program::record_program_counter(|c| c.lazy_reference_interner_hit_count += 1);
            let _ = self.memo.set(Arc::downgrade(&entry.resolved));
            return entry.resolved;
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
            crate::program::record_program_counter(|c| c.lazy_reference_blocked_count += 1);
            return Arc::new(Type::Unknown);
        }
        let creation_before = crate::program::type_creation_snapshot();
        crate::program::record_lazy_reference_expansion_start(
            &self.decl_key,
            &self.snapshot.file_name,
            &self.display,
            LAZY_PEEL_STACK.with(|stack| stack.borrow().len()),
        );
        LAZY_PEEL_STACK.with(|stack| stack.borrow_mut().push(guard_key.clone()));

        // Box the working context so a nested peel keeps the per-frame stack small
        // — the struct is large and a deep (but bounded) library `extends` chain
        // would otherwise overflow the stack with on-stack clones.
        let mut ctx = Box::new((*self.snapshot).clone());
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

        let interned = intern_instantiation(
            &self.snapshot,
            &self.decl_key,
            &self.resolved_arguments,
            resolved_ty,
        );
        let _ = self.memo.set(Arc::downgrade(&interned));
        crate::program::record_lazy_reference_expansion(
            &self.decl_key,
            &self.snapshot.file_name,
            &self.display,
            LAZY_PEEL_STACK.with(|stack| stack.borrow().len()),
            creation_before,
        );
        interned
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
    let snapshot = ctx.lazy_resolution_snapshot();
    let creation_scope = ctx.type_declaration_scope.clone();
    Type::Reference(TypeReference::new(
        reference_id.to_string(),
        display.to_string(),
        resolved_arguments.clone(),
        Arc::new(LazyInstantiation {
            snapshot,
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
    let cache = ctx.program_instantiations.lock().ok()?;
    cache
        .get(key)?
        .iter()
        .find(|entry| entry.arguments == arguments)
        .cloned()
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
