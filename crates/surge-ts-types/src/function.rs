use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Type;
use crate::clone_reason::{TypeCopyReason, current_type_copy_reason};

static FUNCTION_TYPE_PAYLOAD_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FunctionTypeCounters {
    pub function_type_payload_alloc_count: u64,
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
    pub parameters: Vec<Type>,
    pub return_type: Type,
    pub is_variadic: bool,
    pub required_parameter_count: usize,
}

impl Clone for FunctionTypePayload {
    fn clone(&self) -> Self {
        record_function_type_payload_deep_clone_count();
        Self {
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            is_variadic: self.is_variadic,
            required_parameter_count: self.required_parameter_count,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct FunctionType {
    payload: Arc<FunctionTypePayload>,
}

impl FunctionType {
    pub fn new(
        parameters: Vec<Type>,
        return_type: Type,
        is_variadic: bool,
        required_parameter_count: usize,
    ) -> Self {
        record_function_type_payload_alloc_count();
        Self {
            payload: Arc::new(FunctionTypePayload {
                parameters,
                return_type,
                is_variadic,
                required_parameter_count,
            }),
        }
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

    pub fn name(&self) -> String {
        let mut parameters = self.parameters().iter().map(Type::name).collect::<Vec<_>>();

        if self.is_variadic() {
            parameters.push("...args: any[]".to_string());
        }

        let parameters = parameters.join(", ");

        format!("({parameters}) => {}", self.return_type().name())
    }
}

impl Clone for FunctionType {
    fn clone(&self) -> Self {
        record_function_type_handle_copy_count();
        record_function_type_copy_count_for_current_reason();
        record_function_type_clone_count();
        Self {
            payload: self.payload.clone(),
        }
    }
}

pub fn snapshot_function_type_counters() -> FunctionTypeCounters {
    FunctionTypeCounters {
        function_type_payload_alloc_count: FUNCTION_TYPE_PAYLOAD_ALLOC_COUNT
            .load(Ordering::Relaxed),
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
