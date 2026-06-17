use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Type;
use crate::clone_reason::{TypeCopyReason, current_type_copy_reason};

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
    pub types: Vec<Type>,
}

impl Clone for UnionTypePayload {
    fn clone(&self) -> Self {
        record_union_type_payload_deep_clone_count();
        Self {
            types: self.types.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnionType {
    payload: Arc<UnionTypePayload>,
}

impl UnionType {
    pub fn new(types: Vec<Type>) -> Self {
        record_union_type_payload_alloc_count();
        Self {
            payload: Arc::new(UnionTypePayload { types }),
        }
    }

    pub fn payload(&self) -> &UnionTypePayload {
        &self.payload
    }

    pub fn types(&self) -> &[Type] {
        &self.payload.types
    }
}

impl Clone for UnionType {
    fn clone(&self) -> Self {
        record_union_type_handle_copy_count();
        record_union_type_copy_count_for_current_reason();
        Self {
            payload: self.payload.clone(),
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
    let mut flattened = Vec::new();

    for ty in types {
        match ty {
            Type::Union(union) => flattened.extend(union.types().iter().cloned()),
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

    let mut unique = Vec::new();
    for ty in flattened {
        if !unique.contains(&ty) {
            unique.push(ty);
        }
    }

    match unique.len() {
        0 if had_members => Type::Never,
        0 => Type::Unknown,
        1 => unique.into_iter().next().unwrap(),
        _ => Type::Union(UnionType::new(unique)),
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
