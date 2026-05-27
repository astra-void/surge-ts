use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCopyReason {
    Other,
    ExpressionInference,
    CallResolution,
    PropertyCallResolution,
    FunctionBodySetup,
    ReturnChecking,
    ExpectedType,
    SymbolTable,
    ModuleExport,
    ScopeOrContext,
    SubstitutionUnchanged,
    SubstitutionChanged,
    DiagnosticFormatting,
}

thread_local! {
    static CURRENT_TYPE_COPY_REASON: Cell<TypeCopyReason> = const { Cell::new(TypeCopyReason::Other) };
}

pub fn with_type_copy_reason<R>(reason: TypeCopyReason, f: impl FnOnce() -> R) -> R {
    let previous = CURRENT_TYPE_COPY_REASON.with(|cell| cell.replace(reason));
    let result = f();
    CURRENT_TYPE_COPY_REASON.with(|cell| cell.set(previous));
    result
}

pub(crate) fn current_type_copy_reason() -> TypeCopyReason {
    CURRENT_TYPE_COPY_REASON.with(Cell::get)
}
