use std::hash::{Hash, Hasher};

use crate::fx::{FxHasher, PrehashedU64Map};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::clone_reason::{TypeCopyReason, current_type_copy_reason};
use crate::store::{canonical_union_store_enabled, current_program_type_store};
use crate::{Type, TypeListId, UnionTypeId};

static UNION_TYPE_PAYLOAD_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_PAYLOAD_DEEP_CLONE_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_HANDLE_COPY_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_EXPRESSION_INFERENCE_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_CALL_RESOLUTION_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_PROPERTY_CALL_RESOLUTION_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_FUNCTION_BODY_SETUP_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_RETURN_CHECKING_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_EXPECTED_TYPE_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_SYMBOL_TABLE_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_MODULE_EXPORT_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_SCOPE_OR_CONTEXT_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_SUBSTITUTION_UNCHANGED_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_SUBSTITUTION_CHANGED_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_FROM_DIAGNOSTIC_FORMATTING_COUNT: AtomicU64 = AtomicU64::new(0);
static UNION_TYPE_COPY_UNATTRIBUTED_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UnionTypeCounters {
    pub union_type_payload_alloc_count: u64,
    pub union_type_payload_deep_clone_count: u64,
    pub union_type_handle_copy_count: u64,
    pub union_type_copy_from_expression_inference_count: u64,
    pub union_type_copy_from_call_resolution_count: u64,
    pub union_type_copy_from_property_call_resolution_count: u64,
    pub union_type_copy_from_function_body_setup_count: u64,
    pub union_type_copy_from_return_checking_count: u64,
    pub union_type_copy_from_expected_type_count: u64,
    pub union_type_copy_from_symbol_table_count: u64,
    pub union_type_copy_from_module_export_count: u64,
    pub union_type_copy_from_scope_or_context_count: u64,
    pub union_type_copy_from_substitution_unchanged_count: u64,
    pub union_type_copy_from_substitution_changed_count: u64,
    pub union_type_copy_from_diagnostic_formatting_count: u64,
    pub union_type_copy_unattributed_count: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnionTypePayload {
    pub types: Arc<[Type]>,
    pub(crate) list_id: Option<TypeListId>,
}

impl Clone for UnionTypePayload {
    fn clone(&self) -> Self {
        record_union_type_payload_deep_clone_count();
        Self {
            types: self.types.clone(),
            list_id: self.list_id,
        }
    }
}

#[derive(Debug)]
pub struct UnionType {
    payload: Arc<UnionTypePayload>,
    id: Option<UnionTypeId>,
}

impl UnionType {
    pub fn new(types: Vec<Type>) -> Self {
        if canonical_union_store_enabled()
            && let Some(store) = current_program_type_store()
        {
            match store.intern_union(types) {
                Ok((payload, id)) => {
                    return Self {
                        payload,
                        id: Some(id),
                    };
                }
                Err(types) => {
                    record_union_type_payload_alloc_count();
                    return Self {
                        payload: Arc::new(UnionTypePayload {
                            types: types.into(),
                            list_id: None,
                        }),
                        id: None,
                    };
                }
            }
        }
        record_union_type_payload_alloc_count();
        Self {
            payload: Arc::new(UnionTypePayload {
                types: types.into(),
                list_id: None,
            }),
            id: None,
        }
    }

    /// Like [`Self::new`], but probes the interner through borrowed members so
    /// an interner hit (the overwhelmingly common case: 2.4M hits vs 89k
    /// unique unions on tRPC) never deep-clones the member types. Only a miss
    /// — a genuinely new canonical union — materializes the owned member list.
    fn from_borrowed_members(members: &[&Type]) -> Self {
        if canonical_union_store_enabled()
            && let Some(store) = current_program_type_store()
            && let Some((payload, id)) = store.intern_union_borrowed(members)
        {
            return Self {
                payload,
                id: Some(id),
            };
        }
        Self::new(members.iter().map(|ty| (*ty).clone()).collect())
    }

    pub fn payload(&self) -> &UnionTypePayload {
        &self.payload
    }

    pub fn types(&self) -> &[Type] {
        &self.payload.types
    }

    pub fn id(&self) -> Option<UnionTypeId> {
        self.id
    }

    pub fn payload_address(&self) -> usize {
        Arc::as_ptr(&self.payload) as usize
    }

    pub fn member_list_address(&self) -> usize {
        self.payload.types.as_ptr() as usize
    }
}

impl PartialEq for UnionType {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.payload, &other.payload) || self.payload == other.payload
    }
}

impl Eq for UnionType {}

impl Clone for UnionType {
    fn clone(&self) -> Self {
        record_union_type_handle_copy_count();
        record_union_type_copy_count_for_current_reason();
        Self {
            payload: self.payload.clone(),
            id: self.id,
        }
    }
}

pub fn remove_undefined(ty: &Type) -> Type {
    match ty {
        Type::Union(union) => {
            let filtered: Vec<Type> = union
                .types()
                .iter()
                .filter(|t| **t != Type::Undefined)
                .cloned()
                .collect();
            union_type(filtered)
        }
        Type::Undefined => Type::Unknown, // Or whatever makes sense, maybe just return it
        _ => ty.clone(),
    }
}

pub fn remove_nullish(ty: &Type) -> Type {
    match ty {
        Type::Union(union) => {
            let filtered: Vec<Type> = union
                .types()
                .iter()
                .filter(|t| **t != Type::Undefined && **t != Type::Void)
                .cloned()
                .collect();
            union_type(filtered)
        }
        Type::Undefined | Type::Void => Type::Unknown,
        _ => ty.clone(),
    }
}

pub fn union_type(types: Vec<Type>) -> Type {
    // Flatten, simplify, and dedup over *borrowed* members: the members are
    // only cloned when the result is a genuinely new canonical union (see
    // `UnionType::from_borrowed_members`) or the single surviving member.
    let mut flattened: Vec<&Type> = Vec::with_capacity(types.len());

    for ty in &types {
        match ty {
            Type::Union(union) => flattened.extend(union.types().iter()),
            other => flattened.push(other),
        }
    }

    if flattened.iter().any(|ty| matches!(ty, Type::Any)) {
        return Type::Any;
    }

    // `never` is the identity element of union: `T | never` is `T`. Drop it so
    // distributive conditional results (e.g. `Exclude`) collapse cleanly. If
    // every member was `never`, the union itself is `never`.
    let had_members = !flattened.is_empty();
    flattened.retain(|ty| !matches!(ty, Type::Never));

    let unique = dedup_members(flattened);

    match unique.len() {
        0 if had_members => Type::Never,
        0 => Type::Unknown,
        1 => unique[0].clone(),
        _ => Type::Union(UnionType::from_borrowed_members(&unique)),
    }
}

/// Below this size, pairwise `contains` beats the per-member hashing overhead;
/// most unions the checker builds have a handful of members.
const LINEAR_DEDUP_LIMIT: usize = 16;

fn dedup_members<'a>(flattened: Vec<&'a Type>) -> Vec<&'a Type> {
    let mut unique: Vec<&'a Type> = Vec::with_capacity(flattened.len().min(64));
    if flattened.len() <= LINEAR_DEDUP_LIMIT {
        for ty in flattened {
            if !unique.iter().any(|existing| *existing == ty) {
                unique.push(ty);
            }
        }
        return unique;
    }

    // Inline-first buckets: the overflow `Vec` only allocates on a fingerprint
    // collision between distinct members (vanishingly rare with 64-bit keys),
    // so a large union's dedup costs one map allocation instead of one `Vec`
    // per distinct member.
    let mut buckets: PrehashedU64Map<(usize, Vec<usize>)> =
        PrehashedU64Map::with_capacity_and_hasher(flattened.len(), Default::default());
    for ty in flattened {
        match buckets.entry(dedup_key(ty)) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert((unique.len(), Vec::new()));
                unique.push(ty);
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let (first, overflow) = entry.get_mut();
                if unique[*first] == ty || overflow.iter().any(|&index| unique[index] == ty) {
                    continue;
                }
                overflow.push(unique.len());
                unique.push(ty);
            }
        }
    }
    unique
}

fn dedup_key(ty: &Type) -> u64 {
    let mut hasher = FxHasher::default();
    dedup_key_into(ty, &mut hasher, 0);
    hasher.finish()
}

/// Equality-consistent structural digest of a type: `a == b` (under `Type`'s
/// `PartialEq`) implies equal digests, and the digest is never pointer-based
/// (see [`dedup_key_into`]'s invariant). Used by the checker's speculative-check
/// conflict tracking, where a digest collision only causes a spurious (sound)
/// serial recheck, never a missed conflict.
///
/// Much finer than [`dedup_key`]: that key's depth-3 cutoff, name-only object
/// hashing, and empty function arm collide deep-but-distinct instantiation
/// arguments, and in the speculative commit walk each collision falsely marks a
/// valid replay stale (its miss digest matches a published entry whose
/// arguments differ, i.e. one serial checking would also have missed).
/// Measured on tRPC: 212 of 226 stale-replay offender digests were exactly this
/// class. This walker keeps the same consistency invariant — hash only
/// (a subset of) equality-participating fields, structurally, never by pointer.
pub fn type_conflict_digest(ty: &Type) -> u64 {
    let mut hasher = FxHasher::default();
    let mut budget: u32 = FINE_KEY_NODE_BUDGET;
    fine_key_into(ty, &mut hasher, 0, &mut budget);
    hasher.finish()
}

const FINE_KEY_NODE_BUDGET: u32 = 256;
const FINE_KEY_MAX_DEPTH: u8 = 16;
/// Independent per-property budget for object members. Object equality is
/// order-independent (`IndexMap`), so each property's contribution must not
/// depend on iteration order — a shared budget would truncate different
/// properties depending on insertion order and break equality consistency.
const FINE_KEY_PROPERTY_BUDGET: u32 = 64;

fn fine_key_into(ty: &Type, hasher: &mut FxHasher, depth: u8, budget: &mut u32) {
    std::mem::discriminant(ty).hash(hasher);
    if depth >= FINE_KEY_MAX_DEPTH || *budget == 0 {
        return;
    }
    *budget -= 1;
    match ty {
        Type::StringLiteral(value) => value.hash(hasher),
        Type::NumberLiteral(value) => value.value.hash(hasher),
        Type::BooleanLiteral(value) => value.hash(hasher),
        Type::Reference(reference) => {
            // `display` is excluded: `TypeReference` equality is nominal
            // (id + arguments only).
            reference.id.hash(hasher);
            reference.arguments.len().hash(hasher);
            for argument in reference.arguments.iter() {
                fine_key_into(argument, hasher, depth + 1, budget);
            }
        }
        Type::Array(element) => fine_key_into(element, hasher, depth + 1, budget),
        Type::Tuple(elements) => {
            elements.len().hash(hasher);
            for element in elements {
                fine_key_into(element, hasher, depth + 1, budget);
            }
        }
        Type::Union(union) => {
            // `list_id` participates in the derived payload equality; interned
            // structurally, it doubles as a full-depth member-list fingerprint.
            union.payload.list_id.hash(hasher);
            union.types().len().hash(hasher);
            for member in union.types() {
                fine_key_into(member, hasher, depth + 1, budget);
            }
        }
        Type::Object(object) => {
            object.properties.len().hash(hasher);
            // Order-independent map equality: fold per-property digests with a
            // commutative combiner, each walked under its own fixed budget.
            let mut combined: u64 = 0;
            for (name, property) in object.properties.iter() {
                let mut property_hasher = FxHasher::default();
                name.hash(&mut property_hasher);
                property.optional.hash(&mut property_hasher);
                let mut property_budget = FINE_KEY_PROPERTY_BUDGET;
                fine_key_into(
                    &property.ty,
                    &mut property_hasher,
                    depth + 1,
                    &mut property_budget,
                );
                combined = combined.wrapping_add(property_hasher.finish());
            }
            combined.hash(hasher);
            // `call_signature`, `construct_signature`, and `is_intersection`
            // are excluded from `ObjectType` equality and must stay unhashed.
            match &object.string_index_type {
                Some(index) => {
                    1u8.hash(hasher);
                    fine_key_into(index, hasher, depth + 1, budget);
                }
                None => 0u8.hash(hasher),
            }
        }
        Type::Function(function) => {
            let payload = &function.payload;
            payload.parameter_list_id.hash(hasher);
            payload.parameters.len().hash(hasher);
            for parameter in payload.parameters.iter() {
                fine_key_into(parameter, hasher, depth + 1, budget);
            }
            fine_key_into(&payload.return_type, hasher, depth + 1, budget);
            payload.is_variadic.hash(hasher);
            payload.required_parameter_count.hash(hasher);
        }
        _ => {}
    }
}

/// Coarse structural key for union dedup. Invariant: `a == b` (under `Type`'s
/// `PartialEq`) must imply `dedup_key(a) == dedup_key(b)`, so only fields that
/// participate in equality are hashed, and always structurally — never by
/// pointer, since `ObjectType`/`FunctionType`/`UnionType` equality accepts
/// structurally-equal values behind distinct `Arc`s.
fn dedup_key_into(ty: &Type, hasher: &mut FxHasher, depth: u8) {
    std::mem::discriminant(ty).hash(hasher);
    if depth >= 3 {
        return;
    }
    match ty {
        Type::StringLiteral(value) => value.hash(hasher),
        Type::NumberLiteral(value) => value.value.hash(hasher),
        Type::BooleanLiteral(value) => value.hash(hasher),
        Type::Reference(reference) => {
            reference.id.hash(hasher);
            reference.arguments.len().hash(hasher);
            for argument in reference.arguments.iter() {
                dedup_key_into(argument, hasher, depth + 1);
            }
        }
        Type::Array(element) => dedup_key_into(element, hasher, depth + 1),
        Type::Tuple(elements) => {
            elements.len().hash(hasher);
            for element in elements {
                dedup_key_into(element, hasher, depth + 1);
            }
        }
        Type::Union(union) => {
            union.types().len().hash(hasher);
            for member in union.types() {
                dedup_key_into(member, hasher, depth + 1);
            }
        }
        Type::Object(object) => {
            object.properties.len().hash(hasher);
            // IndexMap equality is order-independent, so property names must be
            // mixed with a commutative combiner rather than hashed in iteration
            // order.
            let mut names: u64 = 0;
            for name in object.properties.keys() {
                let mut name_hasher = FxHasher::default();
                name.hash(&mut name_hasher);
                names = names.wrapping_add(name_hasher.finish());
            }
            names.hash(hasher);
            object.string_index_type.is_some().hash(hasher);
        }
        Type::Function(_) => {}
        _ => {}
    }
}

pub fn snapshot_union_type_counters() -> UnionTypeCounters {
    UnionTypeCounters {
        union_type_payload_alloc_count: UNION_TYPE_PAYLOAD_ALLOC_COUNT.load(Ordering::Relaxed),
        union_type_payload_deep_clone_count: UNION_TYPE_PAYLOAD_DEEP_CLONE_COUNT
            .load(Ordering::Relaxed),
        union_type_handle_copy_count: UNION_TYPE_HANDLE_COPY_COUNT.load(Ordering::Relaxed),
        union_type_copy_from_expression_inference_count:
            UNION_TYPE_COPY_FROM_EXPRESSION_INFERENCE_COUNT.load(Ordering::Relaxed),
        union_type_copy_from_call_resolution_count: UNION_TYPE_COPY_FROM_CALL_RESOLUTION_COUNT
            .load(Ordering::Relaxed),
        union_type_copy_from_property_call_resolution_count:
            UNION_TYPE_COPY_FROM_PROPERTY_CALL_RESOLUTION_COUNT.load(Ordering::Relaxed),
        union_type_copy_from_function_body_setup_count:
            UNION_TYPE_COPY_FROM_FUNCTION_BODY_SETUP_COUNT.load(Ordering::Relaxed),
        union_type_copy_from_return_checking_count: UNION_TYPE_COPY_FROM_RETURN_CHECKING_COUNT
            .load(Ordering::Relaxed),
        union_type_copy_from_expected_type_count: UNION_TYPE_COPY_FROM_EXPECTED_TYPE_COUNT
            .load(Ordering::Relaxed),
        union_type_copy_from_symbol_table_count: UNION_TYPE_COPY_FROM_SYMBOL_TABLE_COUNT
            .load(Ordering::Relaxed),
        union_type_copy_from_module_export_count: UNION_TYPE_COPY_FROM_MODULE_EXPORT_COUNT
            .load(Ordering::Relaxed),
        union_type_copy_from_scope_or_context_count: UNION_TYPE_COPY_FROM_SCOPE_OR_CONTEXT_COUNT
            .load(Ordering::Relaxed),
        union_type_copy_from_substitution_unchanged_count:
            UNION_TYPE_COPY_FROM_SUBSTITUTION_UNCHANGED_COUNT.load(Ordering::Relaxed),
        union_type_copy_from_substitution_changed_count:
            UNION_TYPE_COPY_FROM_SUBSTITUTION_CHANGED_COUNT.load(Ordering::Relaxed),
        union_type_copy_from_diagnostic_formatting_count:
            UNION_TYPE_COPY_FROM_DIAGNOSTIC_FORMATTING_COUNT.load(Ordering::Relaxed),
        union_type_copy_unattributed_count: UNION_TYPE_COPY_UNATTRIBUTED_COUNT
            .load(Ordering::Relaxed),
    }
}

pub(crate) fn record_union_type_payload_alloc_count() {
    UNION_TYPE_PAYLOAD_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_union_type_payload_deep_clone_count() {
    UNION_TYPE_PAYLOAD_DEEP_CLONE_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_union_type_handle_copy_count() {
    UNION_TYPE_HANDLE_COPY_COUNT.fetch_add(1, Ordering::Relaxed);
}

fn record_union_type_copy_count_for_current_reason() {
    match current_type_copy_reason() {
        TypeCopyReason::ExpressionInference => {
            UNION_TYPE_COPY_FROM_EXPRESSION_INFERENCE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::CallResolution => {
            UNION_TYPE_COPY_FROM_CALL_RESOLUTION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::PropertyCallResolution => {
            UNION_TYPE_COPY_FROM_PROPERTY_CALL_RESOLUTION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::FunctionBodySetup => {
            UNION_TYPE_COPY_FROM_FUNCTION_BODY_SETUP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ReturnChecking => {
            UNION_TYPE_COPY_FROM_RETURN_CHECKING_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ExpectedType => {
            UNION_TYPE_COPY_FROM_EXPECTED_TYPE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::SymbolTable => {
            UNION_TYPE_COPY_FROM_SYMBOL_TABLE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ModuleExport => {
            UNION_TYPE_COPY_FROM_MODULE_EXPORT_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ScopeOrContext => {
            UNION_TYPE_COPY_FROM_SCOPE_OR_CONTEXT_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::SubstitutionUnchanged => {
            UNION_TYPE_COPY_FROM_SUBSTITUTION_UNCHANGED_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::SubstitutionChanged => {
            UNION_TYPE_COPY_FROM_SUBSTITUTION_CHANGED_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::DiagnosticFormatting => {
            UNION_TYPE_COPY_FROM_DIAGNOSTIC_FORMATTING_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::Other => {
            UNION_TYPE_COPY_UNATTRIBUTED_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NumberLiteralType, Type};

    #[test]
    fn literal_union_dedupes_exact_duplicates() {
        let ty = union_type(vec![
            Type::StringLiteral("ok".to_string()),
            Type::StringLiteral("ok".to_string()),
        ]);

        assert_eq!(ty, Type::StringLiteral("ok".to_string()));
    }

    #[test]
    fn literal_union_display_stable() {
        let ty = union_type(vec![
            Type::StringLiteral("ok".to_string()),
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::BooleanLiteral(true),
        ]);

        assert_eq!(ty.name(), r#""ok" | 1 | true"#);
    }

    #[test]
    fn literal_union_dedupes_string_literals() {
        let ty = union_type(vec![
            Type::StringLiteral("idle".to_string()),
            Type::StringLiteral("idle".to_string()),
        ]);

        assert_eq!(ty, Type::StringLiteral("idle".to_string()));
    }

    #[test]
    fn literal_union_dedupes_number_literals() {
        let ty = union_type(vec![
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
        ]);

        assert_eq!(
            ty,
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            })
        );
    }

    #[test]
    fn literal_union_dedupes_boolean_literals() {
        let ty = union_type(vec![Type::BooleanLiteral(true), Type::BooleanLiteral(true)]);

        assert_eq!(ty, Type::BooleanLiteral(true));
    }

    #[test]
    fn literal_union_preserves_first_seen_order() {
        let ty = union_type(vec![
            Type::StringLiteral("idle".to_string()),
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::BooleanLiteral(true),
            Type::StringLiteral("idle".to_string()),
        ]);

        assert_eq!(ty.name(), r#""idle" | 1 | true"#);
    }

    #[test]
    fn literal_union_does_not_collapse_to_primitive() {
        let ty = union_type(vec![Type::StringLiteral("ok".to_string()), Type::String]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), r#""ok" | string"#);
    }

    #[test]
    fn literal_union_does_not_collapse_string_literal_with_string() {
        let ty = union_type(vec![Type::StringLiteral("idle".to_string()), Type::String]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), r#""idle" | string"#);
    }

    #[test]
    fn literal_union_does_not_collapse_number_literal_with_number() {
        let ty = union_type(vec![
            Type::NumberLiteral(NumberLiteralType {
                value: "1".to_string(),
            }),
            Type::Number,
        ]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), "1 | number");
    }

    #[test]
    fn literal_union_does_not_collapse_boolean_literal_with_boolean() {
        let ty = union_type(vec![Type::BooleanLiteral(true), Type::Boolean]);

        assert!(matches!(ty, Type::Union(_)));
        assert_eq!(ty.name(), "true | boolean");
    }

    #[test]
    fn union_drops_never_members() {
        let ty = union_type(vec![Type::StringLiteral("b".to_string()), Type::Never]);
        assert_eq!(ty, Type::StringLiteral("b".to_string()));
    }

    #[test]
    fn union_of_only_never_is_never() {
        let ty = union_type(vec![Type::Never, Type::Never]);
        assert_eq!(ty, Type::Never);
    }

    #[test]
    fn empty_union_stays_unknown() {
        assert_eq!(union_type(vec![]), Type::Unknown);
    }

    #[test]
    fn literal_union_with_any_collapses_to_any() {
        let ty = union_type(vec![Type::StringLiteral("ok".to_string()), Type::Any]);

        assert_eq!(ty, Type::Any);
    }

    #[test]
    fn large_union_dedup_matches_linear_semantics() {
        // Exceeds LINEAR_DEDUP_LIMIT so the hashed path runs; every member is
        // duplicated once and first-seen order must survive.
        let members: Vec<Type> = (0..40)
            .map(|index| Type::StringLiteral(format!("member-{index}")))
            .collect();
        let mut doubled = members.clone();
        doubled.extend(members.clone());

        let ty = union_type(doubled);
        match &ty {
            Type::Union(union) => assert_eq!(union.types(), members.as_slice()),
            other => panic!("expected union, got {other:?}"),
        }
    }

    #[test]
    fn large_union_dedups_structurally_equal_objects() {
        use crate::{ObjectProperty, ObjectType, PropertyMap};
        use std::sync::Arc;

        // Structurally equal objects behind distinct Arcs must still dedup on
        // the hashed path, matching `Type::eq`.
        let object = || {
            let mut properties = PropertyMap::default();
            properties.insert("value".into(), ObjectProperty::required(Type::String));
            Type::Object(ObjectType {
                properties: Arc::new(properties),
                property_map_id: None,
                string_index_type: None,
                alias_name: None,
                alias_id: None,
                construct_signature: None,
                call_signature: None,
                is_intersection: false,
            })
        };

        let mut members: Vec<Type> = (0..40)
            .map(|index| Type::StringLiteral(format!("member-{index}")))
            .collect();
        members.push(object());
        members.push(object());

        let ty = union_type(members);
        match &ty {
            Type::Union(union) => {
                assert_eq!(union.types().len(), 41);
                assert!(matches!(union.types().last(), Some(Type::Object(_))));
            }
            other => panic!("expected union, got {other:?}"),
        }
    }

    #[test]
    fn literal_union_flattens_nested_literal_unions() {
        let ty = union_type(vec![
            Type::Union(UnionType::new(vec![
                Type::StringLiteral("idle".to_string()),
                Type::NumberLiteral(NumberLiteralType {
                    value: "1".to_string(),
                }),
            ])),
            Type::Union(UnionType::new(vec![
                Type::BooleanLiteral(true),
                Type::StringLiteral("idle".to_string()),
            ])),
        ]);

        assert_eq!(ty.name(), r#""idle" | 1 | true"#);
    }
}
