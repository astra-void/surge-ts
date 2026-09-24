use std::collections::HashSet;
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedType, ParsedTypeParameter};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::context::{CheckerContext, convert_span};
use crate::symbols::TypeDeclarationInfo;

pub(crate) mod cache;
mod diagnostics;
pub(crate) mod interface;
mod resolve;
mod utility;

pub(crate) use cache::*;
pub(crate) use diagnostics::*;
pub(crate) use interface::*;
pub(crate) use resolve::*;
pub(crate) use utility::*;
/// Type-parameter bindings for one resolution context.
///
/// Substitutions are tiny (one entry per type parameter of the declaration
/// being instantiated) but cloned constantly — every lazy reference captures
/// one — so the maps are name-sorted `Arc`-shared vectors: a clone is two
/// refcount bumps, a copy-on-write is a single allocation of small pairs, and
/// iteration order matches the previous `BTreeMap` exactly (sorted by name).
#[derive(Debug, Clone, Default)]
pub(crate) struct TypeParameterSubstitution {
    values: Option<Arc<Vec<(Arc<str>, Type)>>>,
    placeholders: Option<Arc<Vec<Arc<str>>>>,
    /// Type parameters whose inferred literal candidates must not widen: a
    /// constrained parameter (`R extends "json" | "blob"`) decides for itself
    /// whether a literal survives, as tsc's contextual widening does.
    literal_keeping: Option<Arc<Vec<Arc<str>>>>,
    /// Type parameters bound to a *degraded* resolution. A binding is a bare
    /// `Type`, so without this a failed argument reads back clean and the
    /// operators downstream decide from a value surge never resolved. Kept as a
    /// name set beside the bindings, like `placeholders`, so the hot
    /// per-binding tuple does not grow and the common (nothing degraded) case
    /// costs one `None`.
    degraded: Option<Arc<Vec<Arc<str>>>>,
    /// Every candidate a call's inference recorded per type parameter, in
    /// arrival order; the binding is computed from the whole list, as tsc's
    /// `getInferredType` does. Scratch state of one inference, cleared when it
    /// ends so no reference that captures the binding keeps the list alive.
    inference_candidates: Option<Arc<Vec<InferenceRecord>>>,
}

#[derive(Debug, Clone)]
struct InferenceRecord {
    name: Arc<str>,
    candidates: Vec<InferenceCandidate>,
    /// tsc's `InferenceInfo.isFixed`: the binding was read to type a callback's
    /// parameter, and no later candidate changes it.
    fixed: bool,
}

/// A type recorded for a type parameter while a call's arguments are inferred
/// from (tsc's `InferenceInfo.candidates`, or `contraCandidates`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InferenceCandidate {
    pub(crate) ty: Type,
    /// The type of an object or array literal (`isObjectOrArrayLiteralType`).
    pub(crate) literal: bool,
    /// Inferred from a contravariant position: a callback's parameter.
    pub(crate) contravariant: bool,
    /// The parameter the argument was inferred against names the type
    /// parameter at its top level (`InferenceInfo.topLevel`).
    pub(crate) top_level: bool,
    /// Taken from a fresh literal, whose literal types a fixed inference
    /// widens (`getWidenedLiteralType` leaves a regular literal alone).
    pub(crate) fresh: bool,
}

impl TypeParameterSubstitution {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }

    /// Census-only `(address, bytes)` pairs for the `Arc`-shared capture maps,
    /// so retention walks charge each shared map once.
    pub(crate) fn census_shared_captures(&self) -> Vec<(usize, u64)> {
        let mut captures = Vec::new();
        if let Some(values) = &self.values {
            let bytes = values
                .iter()
                .map(|(name, _)| {
                    (name.len()
                        + std::mem::size_of::<Arc<str>>()
                        + std::mem::size_of::<Type>()
                        + 32) as u64
                })
                .sum();
            captures.push((Arc::as_ptr(values) as *const () as usize, bytes));
        }
        if let Some(placeholders) = &self.placeholders {
            let bytes = placeholders
                .iter()
                .map(|name| (name.len() + std::mem::size_of::<Arc<str>>() + 16) as u64)
                .sum();
            captures.push((Arc::as_ptr(placeholders) as *const () as usize, bytes));
        }
        captures
    }
}

impl TypeParameterSubstitution {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set(&mut self, name: String, ty: Type, placeholder: bool) {
        let name: Arc<str> = Arc::from(name.as_str());
        let values = Arc::make_mut(self.values.get_or_insert_with(|| Arc::new(Vec::new())));
        match values.binary_search_by(|(existing, _)| existing.as_ref().cmp(&*name)) {
            Ok(index) => values[index].1 = ty,
            Err(index) => values.insert(index, (name.clone(), ty)),
        }
        if placeholder {
            let placeholders = Arc::make_mut(
                self.placeholders
                    .get_or_insert_with(|| Arc::new(Vec::new())),
            );
            if let Err(index) =
                placeholders.binary_search_by(|existing| existing.as_ref().cmp(&*name))
            {
                placeholders.insert(index, name.clone());
            }
        } else if let Some(placeholders) = self.placeholders.as_mut() {
            if let Ok(index) =
                placeholders.binary_search_by(|existing| existing.as_ref().cmp(&*name))
            {
                Arc::make_mut(placeholders).remove(index);
            }
        }
        if let Some(degraded) = self.degraded.as_mut()
            && let Ok(index) = degraded.binary_search_by(|existing| existing.as_ref().cmp(&*name))
        {
            Arc::make_mut(degraded).remove(index);
        }
    }

    /// Records that `name`'s binding came from a degraded resolution. Call
    /// after the `set`/`insert` that bound it — `set` clears the mark, so a
    /// later clean rebind does not inherit it.
    pub(crate) fn mark_degraded(&mut self, name: &str) {
        let degraded = Arc::make_mut(self.degraded.get_or_insert_with(|| Arc::new(Vec::new())));
        if let Err(index) = degraded.binary_search_by(|existing| existing.as_ref().cmp(name)) {
            degraded.insert(index, Arc::from(name));
        }
    }

    pub(crate) fn is_degraded(&self, name: &str) -> bool {
        self.degraded.as_deref().is_some_and(|degraded| {
            degraded
                .binary_search_by(|existing| existing.as_ref().cmp(name))
                .is_ok()
        })
    }

    pub(crate) fn insert(&mut self, name: String, ty: Type) {
        self.set(name, ty, false);
    }

    pub(crate) fn mark_keeps_literal(&mut self, name: &str) {
        let keeping = Arc::make_mut(
            self.literal_keeping
                .get_or_insert_with(|| Arc::new(Vec::new())),
        );
        if let Err(index) = keeping.binary_search_by(|existing| existing.as_ref().cmp(name)) {
            keeping.insert(index, Arc::from(name));
        }
    }

    pub(crate) fn keeps_literal(&self, name: &str) -> bool {
        self.literal_keeping.as_deref().is_some_and(|keeping| {
            keeping
                .binary_search_by(|existing| existing.as_ref().cmp(name))
                .is_ok()
        })
    }

    pub(crate) fn insert_placeholder(&mut self, name: String, ty: Type) {
        self.set(name, ty, true);
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Type> {
        let values = self.values.as_deref()?;
        values
            .binary_search_by(|(existing, _)| existing.as_ref().cmp(name))
            .ok()
            .map(|index| &values[index].1)
    }

    pub(crate) fn is_placeholder(&self, name: &str) -> bool {
        self.placeholders.as_deref().is_some_and(|placeholders| {
            placeholders
                .binary_search_by(|existing| existing.as_ref().cmp(name))
                .is_ok()
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Arc<str>, &Type)> {
        self.values
            .iter()
            .flat_map(|values| values.iter().map(|(name, ty)| (name, ty)))
    }

    fn inference_record(&self, name: &str) -> Option<&InferenceRecord> {
        self.inference_candidates
            .as_deref()
            .and_then(|records| records.iter().find(|record| record.name.as_ref() == name))
    }

    fn inference_record_mut(&mut self, name: &str) -> &mut InferenceRecord {
        let records = Arc::make_mut(
            self.inference_candidates
                .get_or_insert_with(|| Arc::new(Vec::new())),
        );
        let index = match records.iter().position(|record| record.name.as_ref() == name) {
            Some(index) => index,
            None => {
                records.push(InferenceRecord {
                    name: Arc::from(name),
                    candidates: Vec::new(),
                    fixed: false,
                });
                records.len() - 1
            }
        };
        &mut records[index]
    }

    /// The candidates recorded for `name` so far, in arrival order.
    pub(crate) fn inference_candidates(&self, name: &str) -> &[InferenceCandidate] {
        self.inference_record(name)
            .map_or(&[], |record| record.candidates.as_slice())
    }

    pub(crate) fn push_inference_candidate(&mut self, name: &str, candidate: InferenceCandidate) {
        self.inference_record_mut(name).candidates.push(candidate);
    }

    pub(crate) fn is_inference_fixed(&self, name: &str) -> bool {
        self.inference_record(name).is_some_and(|record| record.fixed)
    }

    pub(crate) fn fix_inference(&mut self, name: &str) {
        self.inference_record_mut(name).fixed = true;
    }

    pub(crate) fn clear_inference_candidates(&mut self) {
        self.inference_candidates = None;
    }

    pub(crate) fn extend(&mut self, other: Self) {
        let Self {
            values,
            placeholders,
            literal_keeping,
            degraded,
            inference_candidates: _,
        } = other;
        if let Some(keeping) = literal_keeping {
            for name in keeping.iter() {
                self.mark_keeps_literal(name);
            }
        }
        let Some(values) = values else {
            return;
        };
        let values = Arc::try_unwrap(values).unwrap_or_else(|values| (*values).clone());
        let placeholders = placeholders
            .map(|placeholders| {
                Arc::try_unwrap(placeholders).unwrap_or_else(|placeholders| (*placeholders).clone())
            })
            .unwrap_or_default();

        for (name, ty) in values {
            let is_placeholder = placeholders
                .binary_search_by(|existing| existing.as_ref().cmp(&*name))
                .is_ok();
            self.set(name.as_ref().to_string(), ty, is_placeholder);
        }
        // After the `set` calls, which clear stale marks.
        if let Some(degraded) = degraded {
            for name in degraded.iter() {
                self.mark_degraded(name);
            }
        }
    }
}

pub(crate) fn report_duplicate_type_parameters(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) {
    let mut seen = HashSet::new();

    for type_parameter in type_parameters {
        if !seen.insert(type_parameter.name.clone()) {
            let mut diagnostic = Diagnostic::surge_duplicate_type_parameter(
                type_parameter.name.clone(),
                ctx.file_name.clone(),
            );

            if let Some(span) = type_parameter.name_span.or(type_parameter.span) {
                diagnostic = diagnostic.with_span(convert_span(span));
            }

            ctx.push(diagnostic);
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedType {
    ty: Type,
    had_error: bool,
}

impl ResolvedType {
    pub(crate) fn had_error(&self) -> bool {
        self.had_error
    }

    pub(crate) fn into_ty(self) -> Type {
        self.ty
    }
}

pub(crate) fn map_parsed_type(parsed_type: ParsedType, ctx: &mut CheckerContext) -> Type {
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        map_parsed_type_with_substitution(parsed_type, ctx, &TypeParameterSubstitution::new())
    })
}

pub(crate) fn map_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> Type {
    try_map_parsed_type_with_substitution(parsed_type, ctx, substitution).ty
}

/// `map_parsed_type_with_substitution` that keeps the resolution's `had_error`
/// flag. A caller that acts on the resolved shape rather than merely carrying
/// it — deciding a constraint, say — needs to know the shape is degraded.
pub(crate) fn try_map_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolving = Vec::new();
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        resolve_parsed_type(
            parsed_type,
            ctx,
            &mut resolving,
            &merged_type_parameter_substitution(ctx, substitution),
        )
    })
}

pub(crate) fn merged_type_parameter_substitution(
    ctx: &CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> TypeParameterSubstitution {
    let mut merged = TypeParameterSubstitution::new();

    for scope in &ctx.type_parameter_scopes {
        for (name, ty) in scope {
            // A scope holds a declaration's own type variables (or the sentinel)
            // while it is resolved or checked; any other type is a call's type
            // argument its body is read under (`instantiated_body_return`).
            if matches!(ty, Type::TypeParameter(_) | Type::Unknown) {
                merged.insert_placeholder(name.clone(), ty.clone());
            } else {
                merged.insert(name.clone(), ty.clone());
            }
        }
    }

    merged.extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionUnchanged));

    merged
}

pub(crate) fn validate_local_type_declaration(
    declaration: &TypeDeclarationInfo,
    ctx: &mut CheckerContext,
) {
    match declaration {
        TypeDeclarationInfo::Alias(alias) => {
            let mut substitution = TypeParameterSubstitution::new();
            for type_parameter in &alias.body.type_parameters {
                substitution.insert_placeholder(
                    type_parameter.name.clone(),
                    Type::type_parameter(&type_parameter.name),
                );
            }

            let mut resolving = Vec::new();
            // A body that degrades is *not* grounds for rolling back what it
            // reported: the report is usually the reason it degraded. `type
            // PickValue<T, K> = T[K]` reports TS2536 and yields the sentinel,
            // and so do the TS2493, TS2686 and TS2304 declarations the span and
            // program tests pin — rolling those back dropped twelve of them.
            // Two false positives this used to mask are recorded in
            // CURRENT_STATUS; each wants fixing where it is raised, not here.
            with_type_declaration_scope(&alias.resolution_scope, ctx, |ctx| {
                with_file_name(ctx, &alias.file_name, |ctx| {
                    // Register the parameter constraints so indexed access through
                    // a constrained parameter (`T extends …`) is not falsely
                    // flagged. Placeholder detection still flows through the
                    // substitution above.
                    ctx.push_type_parameter_scope(&alias.body.type_parameters, None);
                    resolve_parsed_type_with_substitution(
                        alias.body.ty.clone(),
                        ctx,
                        &mut resolving,
                        &substitution,
                    );
                    ctx.pop_type_parameter_scope();
                })
            });
        }
        TypeDeclarationInfo::Interface(interface) => {
            let mut substitution = TypeParameterSubstitution::new();
            for type_parameter in &interface.body.type_parameters {
                substitution.insert_placeholder(
                    type_parameter.name.clone(),
                    Type::type_parameter(&type_parameter.name),
                );
            }

            let mut resolving = Vec::new();
            let outer_class_heritage = std::mem::replace(
                &mut ctx.resolving_class_heritage,
                interface.is_class_instance,
            );
            with_type_declaration_scope(&interface.resolution_scope, ctx, |ctx| {
                with_file_name(ctx, &interface.file_name, |ctx| {
                    ctx.push_type_parameter_scope(&interface.body.type_parameters, None);
                    resolve_interface_declaration(
                        &interface.body.extends,
                        &interface.body.members,
                        interface.body.string_index_type.as_ref(),
                        interface.body.number_index_type.as_ref(),
                        interface.body.call_signature.as_ref(),
                        &interface.body.call_signature_overloads,
                        &interface.body.construct_signatures,
                        ctx,
                        &mut resolving,
                        &substitution,
                        None,
                        None,
                        None,
                        None,
                        (!interface.body.fragment_scopes.is_empty()).then(|| &*interface.body),
                    );
                    ctx.pop_type_parameter_scope();
                })
            });
            ctx.resolving_class_heritage = outer_class_heritage;
        }
    }
}

fn with_file_name<R>(
    ctx: &mut CheckerContext,
    file_name: &str,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let current_file_name = ctx.file_name.clone();
    let crossed_file = current_file_name != file_name;
    ctx.set_file_name(file_name.to_string());
    if crossed_file {
        ctx.cross_file_resolution_depth += 1;
    }
    let result = f(ctx);
    if crossed_file {
        ctx.cross_file_resolution_depth -= 1;
    }
    ctx.set_file_name(current_file_name);
    result
}

pub(crate) fn with_type_declaration_scope<R>(
    type_declaration_scope: &Option<Arc<crate::symbols::TypeDeclarationScope>>,
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let saved_type_declaration_scope = ctx.type_declaration_scope.clone();

    if let Some(type_declaration_scope) = type_declaration_scope {
        ctx.type_declaration_scope = Some(type_declaration_scope.clone());
    }

    let result = f(ctx);
    ctx.type_declaration_scope = saved_type_declaration_scope;
    result
}
