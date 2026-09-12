use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::clone_reason::{TypeCopyReason, current_type_copy_reason};
use crate::store::{canonical_function_store_enabled, current_program_type_store};
use crate::{FunctionTypeId, Type, TypeListId};

static FUNCTION_TYPE_PAYLOAD_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_PAYLOAD_ALLOC_BY_REASON: [AtomicU64; 13] = [const { AtomicU64::new(0) }; 13];
static FUNCTION_TYPE_PAYLOAD_ALLOC_BY_EXPANSION_REASON: [AtomicU64; 42] =
    [const { AtomicU64::new(0) }; 42];
static FUNCTION_TYPE_PAYLOAD_DEEP_CLONE_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_HANDLE_COPY_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_CLONE_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_EXPRESSION_INFERENCE_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_CALL_RESOLUTION_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_PROPERTY_CALL_RESOLUTION_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_FUNCTION_BODY_SETUP_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_RETURN_CHECKING_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_EXPECTED_TYPE_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_SYMBOL_TABLE_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_MODULE_EXPORT_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_SCOPE_OR_CONTEXT_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_SUBSTITUTION_UNCHANGED_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_SUBSTITUTION_CHANGED_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_FROM_DIAGNOSTIC_FORMATTING_COUNT: AtomicU64 = AtomicU64::new(0);
static FUNCTION_TYPE_COPY_UNATTRIBUTED_COUNT: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static CURRENT_FUNCTION_TYPE_EXPANSION_REASON: Cell<usize> = const { Cell::new(41) };
}

pub fn replace_function_type_expansion_reason(reason: usize) -> usize {
    CURRENT_FUNCTION_TYPE_EXPANSION_REASON.replace(reason)
}

pub fn snapshot_function_type_payload_alloc_by_expansion_reason() -> [u64; 42] {
    std::array::from_fn(|index| {
        FUNCTION_TYPE_PAYLOAD_ALLOC_BY_EXPANSION_REASON[index].load(Ordering::Relaxed)
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FunctionTypeCounters {
    pub function_type_payload_alloc_count: u64,
    pub function_type_payload_alloc_by_reason: [u64; 13],
    pub function_type_payload_deep_clone_count: u64,
    pub function_type_handle_copy_count: u64,
    pub function_type_clone_count: u64,
    pub function_type_copy_from_expression_inference_count: u64,
    pub function_type_copy_from_call_resolution_count: u64,
    pub function_type_copy_from_property_call_resolution_count: u64,
    pub function_type_copy_from_function_body_setup_count: u64,
    pub function_type_copy_from_return_checking_count: u64,
    pub function_type_copy_from_expected_type_count: u64,
    pub function_type_copy_from_symbol_table_count: u64,
    pub function_type_copy_from_module_export_count: u64,
    pub function_type_copy_from_scope_or_context_count: u64,
    pub function_type_copy_from_substitution_unchanged_count: u64,
    pub function_type_copy_from_substitution_changed_count: u64,
    pub function_type_copy_from_diagnostic_formatting_count: u64,
    pub function_type_copy_unattributed_count: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FunctionTypePayload {
    pub parameters: Arc<[Type]>,
    pub(crate) parameter_list_id: Option<TypeListId>,
    pub return_type: Type,
    pub is_variadic: bool,
    pub required_parameter_count: usize,
    pub(crate) name_memo: crate::name_memo::NameMemo,
}

impl Clone for FunctionTypePayload {
    fn clone(&self) -> Self {
        record_function_type_payload_deep_clone_count();
        Self {
            parameters: self.parameters.clone(),
            parameter_list_id: self.parameter_list_id,
            return_type: self.return_type.clone(),
            is_variadic: self.is_variadic,
            required_parameter_count: self.required_parameter_count,
            name_memo: self.name_memo.clone(),
        }
    }
}

#[derive(Debug)]
pub struct FunctionType {
    pub(crate) payload: Arc<FunctionTypePayload>,
    id: Option<FunctionTypeId>,
    /// Written parameter names, for display only. tsc renders a function type as
    /// `(value: string) => void`, but names play no part in assignability, so
    /// they live on the handle rather than the interned payload: two signatures
    /// that differ only in parameter names still share one canonical payload.
    parameter_names: Option<Arc<[Option<Arc<str>>]>>,
    /// Rendered type-parameter list (`<T extends FieldBag = FieldBag>`), which
    /// tsc prefixes to a generic signature. Display-only, for the same reason.
    type_parameter_head: Option<Arc<str>>,
    /// The type-alias name this signature was written as (`type Fn = (x: string)
    /// => void`), which tsc displays instead of the structural form.
    alias_name: Option<Arc<str>>,
    /// The checker's record of the declaration this value-level signature came
    /// from (`typeof fn` over a generic `declare function fn<T>`), so a call
    /// through a property typed by it can still instantiate the declared type
    /// parameters. Opaque here: this crate has no declaration table. Handle
    /// metadata like the names above — never part of the payload, identity, or
    /// equality.
    declaration: Option<Arc<dyn std::any::Any + Send + Sync>>,
}

impl FunctionType {
    pub(crate) fn from_canonical_parts(
        payload: Arc<FunctionTypePayload>,
        id: FunctionTypeId,
    ) -> Self {
        Self {
            payload,
            id: Some(id),
            parameter_names: None,
            type_parameter_head: None,
            alias_name: None,
            declaration: None,
        }
    }

    pub fn new(
        parameters: Vec<Type>,
        return_type: Type,
        is_variadic: bool,
        required_parameter_count: usize,
    ) -> Self {
        if canonical_function_store_enabled()
            && let Some(store) = current_program_type_store()
        {
            match store.intern_function(
                parameters,
                return_type,
                is_variadic,
                required_parameter_count,
            ) {
                Ok((payload, id)) => {
                    return Self {
                        payload,
                        id: Some(id),
                        parameter_names: None,
                        type_parameter_head: None,
                        alias_name: None,
                        declaration: None,
                    };
                }
                Err((parameters, return_type)) => {
                    store.record_function_fallback();
                    record_function_type_payload_alloc_count();
                    return Self {
                        payload: Arc::new(FunctionTypePayload {
                            parameters: parameters.into(),
                            parameter_list_id: None,
                            return_type,
                            is_variadic,
                            required_parameter_count,
                            name_memo: crate::name_memo::NameMemo::default(),
                        }),
                        id: None,
                        parameter_names: None,
                        type_parameter_head: None,
                        alias_name: None,
                        declaration: None,
                    };
                }
            }
        }
        record_function_type_payload_alloc_count();
        Self {
            payload: Arc::new(FunctionTypePayload {
                parameters: parameters.into(),
                parameter_list_id: None,
                return_type,
                is_variadic,
                required_parameter_count,
                name_memo: crate::name_memo::NameMemo::default(),
            }),
            id: None,
            parameter_names: None,
            type_parameter_head: None,
            alias_name: None,
            declaration: None,
        }
    }

    /// Attaches the written parameter names for display. Ignored when the count
    /// does not match the parameter list, and dropped entirely when no position
    /// carries a name, so an unnamed signature keeps the memoized rendering.
    pub fn with_parameter_names(mut self, names: Vec<Option<Arc<str>>>) -> Self {
        if names.len() == self.payload.parameters.len()
            && names.iter().any(std::option::Option::is_some)
        {
            self.parameter_names = Some(names.into());
        }
        self
    }

    pub fn parameter_names(&self) -> Option<&[Option<Arc<str>>]> {
        self.parameter_names.as_deref()
    }

    /// Attaches the rendered type-parameter list, without the angle brackets.
    pub fn with_type_parameter_head(mut self, head: Option<String>) -> Self {
        self.type_parameter_head = head.filter(|head| !head.is_empty()).map(Arc::from);
        self
    }

    /// Names this signature by the alias it was written as, for display only.
    pub fn with_alias_name(mut self, name: impl Into<Arc<str>>) -> Self {
        self.alias_name = Some(name.into());
        self
    }

    pub fn alias_name(&self) -> Option<&str> {
        self.alias_name.as_deref()
    }

    /// Attaches the declaration record this signature was read from.
    pub fn with_declaration(mut self, declaration: Arc<dyn std::any::Any + Send + Sync>) -> Self {
        self.declaration = Some(declaration);
        self
    }

    pub fn declaration(&self) -> Option<&(dyn std::any::Any + Send + Sync)> {
        self.declaration.as_deref()
    }

    pub fn payload(&self) -> &FunctionTypePayload {
        &self.payload
    }

    pub fn parameters(&self) -> &[Type] {
        &self.payload.parameters
    }

    pub fn return_type(&self) -> &Type {
        &self.payload.return_type
    }

    pub fn is_variadic(&self) -> bool {
        self.payload.is_variadic
    }

    pub fn required_parameter_count(&self) -> usize {
        self.payload.required_parameter_count
    }

    pub fn id(&self) -> Option<FunctionTypeId> {
        self.id
    }

    pub fn parameter_list_id(&self) -> Option<TypeListId> {
        self.payload.parameter_list_id
    }

    pub fn payload_address(&self) -> usize {
        Arc::as_ptr(&self.payload) as usize
    }

    /// A weak handle to the shared payload, for identity-keyed memo entries
    /// that must pin the allocation so its address cannot be reused.
    pub fn payload_weak(&self) -> std::sync::Weak<FunctionTypePayload> {
        Arc::downgrade(&self.payload)
    }

    pub fn parameter_list_address(&self) -> usize {
        self.payload.parameters.as_ptr() as usize
    }

    pub fn name(&self) -> String {
        if let Some(alias_name) = self.alias_name.as_deref() {
            return alias_name.to_string();
        }
        // The memo lives on the shared payload, so a handle-local rendering must
        // not be written into it.
        if self.parameter_names.is_some() || self.type_parameter_head.is_some() {
            return self.render_name();
        }
        self.payload
            .name_memo
            .get_or_render(|| self.render_name())
            .to_string()
    }

    fn render_name(&self) -> String {
        let names = self.parameter_names();
        let mut parameters = self
            .parameters()
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let optional = index >= self.required_parameter_count();
                match names.and_then(|names| names[index].as_deref()) {
                    Some(name) if optional => format!("{name}?: {}", parameter.name()),
                    Some(name) => format!("{name}: {}", parameter.name()),
                    None => parameter.name(),
                }
            })
            .collect::<Vec<_>>();

        if self.is_variadic() {
            parameters.push("...args: any[]".to_string());
        }

        let head = match self.type_parameter_head.as_deref() {
            Some(head) => format!("<{head}>"),
            None => String::new(),
        };
        format!(
            "{head}({}) => {}",
            parameters.join(", "),
            self.return_type().name()
        )
    }
}

impl PartialEq for FunctionType {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.payload, &other.payload) || self.payload == other.payload
    }
}

impl Eq for FunctionType {}

impl Clone for FunctionType {
    fn clone(&self) -> Self {
        record_function_type_handle_copy_count();
        record_function_type_copy_count_for_current_reason();
        record_function_type_clone_count();
        Self {
            payload: self.payload.clone(),
            id: self.id,
            parameter_names: self.parameter_names.clone(),
            type_parameter_head: self.type_parameter_head.clone(),
            alias_name: self.alias_name.clone(),
            declaration: self.declaration.clone(),
        }
    }
}

pub fn snapshot_function_type_counters() -> FunctionTypeCounters {
    FunctionTypeCounters {
        function_type_payload_alloc_count: FUNCTION_TYPE_PAYLOAD_ALLOC_COUNT
            .load(Ordering::Relaxed),
        function_type_payload_alloc_by_reason: std::array::from_fn(|index| {
            FUNCTION_TYPE_PAYLOAD_ALLOC_BY_REASON[index].load(Ordering::Relaxed)
        }),
        function_type_payload_deep_clone_count: FUNCTION_TYPE_PAYLOAD_DEEP_CLONE_COUNT
            .load(Ordering::Relaxed),
        function_type_handle_copy_count: FUNCTION_TYPE_HANDLE_COPY_COUNT.load(Ordering::Relaxed),
        function_type_clone_count: FUNCTION_TYPE_CLONE_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_expression_inference_count:
            FUNCTION_TYPE_COPY_FROM_EXPRESSION_INFERENCE_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_call_resolution_count:
            FUNCTION_TYPE_COPY_FROM_CALL_RESOLUTION_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_property_call_resolution_count:
            FUNCTION_TYPE_COPY_FROM_PROPERTY_CALL_RESOLUTION_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_function_body_setup_count:
            FUNCTION_TYPE_COPY_FROM_FUNCTION_BODY_SETUP_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_return_checking_count:
            FUNCTION_TYPE_COPY_FROM_RETURN_CHECKING_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_expected_type_count: FUNCTION_TYPE_COPY_FROM_EXPECTED_TYPE_COUNT
            .load(Ordering::Relaxed),
        function_type_copy_from_symbol_table_count: FUNCTION_TYPE_COPY_FROM_SYMBOL_TABLE_COUNT
            .load(Ordering::Relaxed),
        function_type_copy_from_module_export_count: FUNCTION_TYPE_COPY_FROM_MODULE_EXPORT_COUNT
            .load(Ordering::Relaxed),
        function_type_copy_from_scope_or_context_count:
            FUNCTION_TYPE_COPY_FROM_SCOPE_OR_CONTEXT_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_substitution_unchanged_count:
            FUNCTION_TYPE_COPY_FROM_SUBSTITUTION_UNCHANGED_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_substitution_changed_count:
            FUNCTION_TYPE_COPY_FROM_SUBSTITUTION_CHANGED_COUNT.load(Ordering::Relaxed),
        function_type_copy_from_diagnostic_formatting_count:
            FUNCTION_TYPE_COPY_FROM_DIAGNOSTIC_FORMATTING_COUNT.load(Ordering::Relaxed),
        function_type_copy_unattributed_count: FUNCTION_TYPE_COPY_UNATTRIBUTED_COUNT
            .load(Ordering::Relaxed),
    }
}

pub(crate) fn record_function_type_payload_alloc_count() {
    FUNCTION_TYPE_PAYLOAD_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
    FUNCTION_TYPE_PAYLOAD_ALLOC_BY_REASON[type_copy_reason_index(current_type_copy_reason())]
        .fetch_add(1, Ordering::Relaxed);
    let expansion_reason = CURRENT_FUNCTION_TYPE_EXPANSION_REASON.get();
    FUNCTION_TYPE_PAYLOAD_ALLOC_BY_EXPANSION_REASON[expansion_reason]
        .fetch_add(1, Ordering::Relaxed);
}

fn type_copy_reason_index(reason: TypeCopyReason) -> usize {
    match reason {
        TypeCopyReason::Other => 0,
        TypeCopyReason::ExpressionInference => 1,
        TypeCopyReason::CallResolution => 2,
        TypeCopyReason::PropertyCallResolution => 3,
        TypeCopyReason::FunctionBodySetup => 4,
        TypeCopyReason::ReturnChecking => 5,
        TypeCopyReason::ExpectedType => 6,
        TypeCopyReason::SymbolTable => 7,
        TypeCopyReason::ModuleExport => 8,
        TypeCopyReason::ScopeOrContext => 9,
        TypeCopyReason::SubstitutionUnchanged => 10,
        TypeCopyReason::SubstitutionChanged => 11,
        TypeCopyReason::DiagnosticFormatting => 12,
    }
}

pub(crate) fn record_function_type_payload_deep_clone_count() {
    FUNCTION_TYPE_PAYLOAD_DEEP_CLONE_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_function_type_handle_copy_count() {
    FUNCTION_TYPE_HANDLE_COPY_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_function_type_clone_count() {
    FUNCTION_TYPE_CLONE_COUNT.fetch_add(1, Ordering::Relaxed);
}

fn record_function_type_copy_count_for_current_reason() {
    match current_type_copy_reason() {
        TypeCopyReason::ExpressionInference => {
            FUNCTION_TYPE_COPY_FROM_EXPRESSION_INFERENCE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::CallResolution => {
            FUNCTION_TYPE_COPY_FROM_CALL_RESOLUTION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::PropertyCallResolution => {
            FUNCTION_TYPE_COPY_FROM_PROPERTY_CALL_RESOLUTION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::FunctionBodySetup => {
            FUNCTION_TYPE_COPY_FROM_FUNCTION_BODY_SETUP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ReturnChecking => {
            FUNCTION_TYPE_COPY_FROM_RETURN_CHECKING_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ExpectedType => {
            FUNCTION_TYPE_COPY_FROM_EXPECTED_TYPE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::SymbolTable => {
            FUNCTION_TYPE_COPY_FROM_SYMBOL_TABLE_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ModuleExport => {
            FUNCTION_TYPE_COPY_FROM_MODULE_EXPORT_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::ScopeOrContext => {
            FUNCTION_TYPE_COPY_FROM_SCOPE_OR_CONTEXT_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::SubstitutionUnchanged => {
            FUNCTION_TYPE_COPY_FROM_SUBSTITUTION_UNCHANGED_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::SubstitutionChanged => {
            FUNCTION_TYPE_COPY_FROM_SUBSTITUTION_CHANGED_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::DiagnosticFormatting => {
            FUNCTION_TYPE_COPY_FROM_DIAGNOSTIC_FORMATTING_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        TypeCopyReason::Other => {
            FUNCTION_TYPE_COPY_UNATTRIBUTED_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_type_name_no_params() {
        let ty = FunctionType::new(vec![], Type::String, false, 0);

        assert_eq!(ty.name(), "() => string");
    }

    #[test]
    fn function_type_name_one_param() {
        let ty = FunctionType::new(vec![Type::String], Type::Number, false, 1);

        assert_eq!(ty.name(), "(string) => number");
    }

    #[test]
    fn function_type_name_multiple_params() {
        let ty = FunctionType::new(
            vec![Type::String, Type::Number, Type::Boolean],
            Type::Void,
            false,
            3,
        );

        assert_eq!(ty.name(), "(string, number, boolean) => void");
    }

    #[test]
    fn function_type_name_nested_parameter() {
        let ty = FunctionType::new(
            vec![Type::Function(FunctionType::new(
                vec![Type::String],
                Type::Number,
                false,
                1,
            ))],
            Type::Void,
            false,
            1,
        );

        assert_eq!(ty.name(), "((string) => number) => void");
    }

    #[test]
    fn function_type_name_nested_return() {
        let ty = FunctionType::new(
            vec![Type::String],
            Type::Function(FunctionType::new(
                vec![Type::Number],
                Type::Boolean,
                false,
                1,
            )),
            false,
            1,
        );

        assert_eq!(ty.name(), "(string) => (number) => boolean");
    }
}
