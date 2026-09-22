use std::fmt;

use crate::render::render_with_span;
use crate::{DiagnosticCategory, DiagnosticCode, DiagnosticDescriptor};

#[derive(Debug, Clone)]
pub enum DiagnosticArg {
    Str(String),
    Usize(usize),
}

impl fmt::Display for DiagnosticArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticArg::Str(value) => f.write_str(value),
            DiagnosticArg::Usize(value) => write!(f, "{value}"),
        }
    }
}

impl From<String> for DiagnosticArg {
    fn from(value: String) -> Self {
        Self::Str(value)
    }
}

impl From<&str> for DiagnosticArg {
    fn from(value: &str) -> Self {
        Self::Str(value.to_string())
    }
}

impl From<usize> for DiagnosticArg {
    fn from(value: usize) -> Self {
        Self::Usize(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub file_name: String,
    pub span: Option<TextSpan>,
    pub severity: DiagnosticCategory,
}

impl Diagnostic {
    pub fn new(
        code: DiagnosticCode,
        message: impl Into<String>,
        file_name: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            file_name: file_name.into(),
            span: None,
            severity: DiagnosticCategory::Error,
        }
    }

    pub fn from_descriptor(
        descriptor: &'static DiagnosticDescriptor,
        args: impl Into<Vec<DiagnosticArg>>,
        file_name: impl Into<String>,
    ) -> Self {
        let args = args.into();
        debug_assert_eq!(args.len(), descriptor.argument_count);

        Self::new(
            descriptor.diagnostic_code(),
            format_message(descriptor.message_template, &args),
            file_name,
        )
        .with_severity(descriptor.category)
    }

    pub fn with_span(mut self, span: TextSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_severity(mut self, severity: DiagnosticCategory) -> Self {
        self.severity = severity;
        self
    }

    pub fn render(&self, source_text: &str) -> String {
        match self.span {
            Some(span) => {
                render_with_span(self, source_text, &crate::LineIndex::new(source_text), span)
            }
            None => self.to_string(),
        }
    }

    /// Like [`render`](Self::render), but reuses a caller-built [`crate::LineIndex`] so
    /// rendering many diagnostics against one source does not rescan the file
    /// per diagnostic.
    pub fn render_with_line_index(
        &self,
        source_text: &str,
        line_index: &crate::LineIndex,
    ) -> String {
        match self.span {
            Some(span) => render_with_span(self, source_text, line_index, span),
            None => self.to_string(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}[{}]: {}",
            self.severity.label(),
            self.code,
            self.message
        )?;
        write!(f, " --> {}", self.file_name)
    }
}

/// Fills `{N}` placeholders. Braces around anything but digits are literal
/// text (TS1202's `import {a} from "mod"`).
fn format_message(template: &str, args: &[DiagnosticArg]) -> String {
    let mut formatted = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        formatted.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        let placeholder = (digits > 0 && after.as_bytes().get(digits) == Some(&b'}'))
            .then(|| after[..digits].parse::<usize>().ok())
            .flatten();
        match placeholder {
            Some(index) => {
                match args.get(index) {
                    Some(arg) => formatted.push_str(&arg.to_string()),
                    None => formatted.push_str(&rest[open..open + digits + 2]),
                }
                rest = &after[digits + 1..];
            }
            None => {
                formatted.push('{');
                rest = after;
            }
        }
    }
    formatted.push_str(rest);
    formatted
}
