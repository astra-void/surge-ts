#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCategory {
    Error,
    Warning,
    Suggestion,
    Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSupport {
    CatalogOnly,
    Emitted,
}

#[derive(Debug, Clone, Copy)]
pub struct TypeScriptDiagnosticDefinition {
    pub code: u32,
    pub key: &'static str,
    pub category: DiagnosticCategory,
    pub message_template: &'static str,
    pub argument_count: usize,
    pub support: DiagnosticSupport,
}
