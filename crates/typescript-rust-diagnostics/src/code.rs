use std::fmt;

#[derive(Debug, Clone)]
pub enum DiagnosticCode {
    TypeScript(u32),
    Custom(&'static str),
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticCode::TypeScript(code) => write!(f, "TS{code}"),
            DiagnosticCode::Custom(code) => f.write_str(code),
        }
    }
}
