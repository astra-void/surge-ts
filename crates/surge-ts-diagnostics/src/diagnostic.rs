use std::fmt;

use crate::render::render_with_span;
use crate::{DiagnosticCode, DiagnosticDescriptor};

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
    }

    pub fn with_span(mut self, span: TextSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn render(&self, source_text: &str) -> String {
        match self.span {
            Some(span) => render_with_span(self, source_text, span),
            None => self.to_string(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error[{}]: {}", self.code, self.message)?;
        write!(f, " --> {}", self.file_name)
    }
}

fn format_message(template: &str, args: &[DiagnosticArg]) -> String {
    let mut formatted = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            formatted.push(ch);
            continue;
        }

        let mut index_text = String::new();
        while let Some(next) = chars.peek().copied() {
            if next == '}' {
                chars.next();
                break;
            }

            if next.is_ascii_digit() {
                index_text.push(next);
                chars.next();
                continue;
            }

            formatted.push('{');
            formatted.push_str(&index_text);
            formatted.push(next);
            chars.next();
            index_text.clear();
            continue;
        }

        if index_text.is_empty() {
            formatted.push('{');
            continue;
        }

        match index_text.parse::<usize>() {
            Ok(index) => match args.get(index) {
                Some(arg) => formatted.push_str(&arg.to_string()),
                None => {
                    formatted.push('{');
                    formatted.push_str(&index_text);
                    formatted.push('}');
                }
            },
            Err(_) => {
                formatted.push('{');
                formatted.push_str(&index_text);
                formatted.push('}');
            }
        }
    }

    formatted
}
