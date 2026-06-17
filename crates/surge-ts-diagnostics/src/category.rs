use crate::code::DiagnosticCode;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    TypeScript,
    TypeScriptRust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticDescriptor {
    pub code: &'static str,
    pub number: Option<u32>,
    pub source: DiagnosticSource,
    pub category: DiagnosticCategory,
    pub message_template: &'static str,
    pub argument_count: usize,
    pub support: DiagnosticSupport,
}

impl DiagnosticDescriptor {
    pub fn diagnostic_code(self) -> DiagnosticCode {
        match self.number {
            Some(number) => DiagnosticCode::TypeScript(number),
            None => DiagnosticCode::Custom(self.code),
        }
    }
}
