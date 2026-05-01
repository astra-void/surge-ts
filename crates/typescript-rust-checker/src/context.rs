use typescript_rust_diagnostics::{Diagnostic, TextSpan as DiagnosticTextSpan};
use typescript_rust_syntax::TextSpan as SyntaxTextSpan;

use crate::symbols::SymbolTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckerOptions {
    pub no_implicit_any: bool,
}

impl Default for CheckerOptions {
    fn default() -> Self {
        Self {
            no_implicit_any: false,
        }
    }
}

pub(crate) struct CheckerContext {
    pub(crate) file_name: String,
    pub(crate) options: CheckerOptions,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) symbols: SymbolTable,
}

impl CheckerContext {
    pub(crate) fn new(file_name: String, options: CheckerOptions) -> Self {
        Self {
            file_name,
            options,
            diagnostics: Vec::new(),
            symbols: SymbolTable::new(),
        }
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

pub(crate) fn convert_span(span: SyntaxTextSpan) -> DiagnosticTextSpan {
    DiagnosticTextSpan {
        start: span.start,
        end: span.end,
    }
}
