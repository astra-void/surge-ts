use typescript_rust_diagnostics::{Diagnostic, TextSpan as DiagnosticTextSpan};
use typescript_rust_syntax::TextSpan as SyntaxTextSpan;

use crate::symbols::{SymbolTable, TypeDeclarationTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckerOptions {
    pub no_implicit_any: bool,
    pub stub_external_modules: bool,
}

impl Default for CheckerOptions {
    fn default() -> Self {
        Self {
            no_implicit_any: false,
            stub_external_modules: false,
        }
    }
}

use crate::modules::ModuleExportTable;

pub(crate) struct CheckerContext {
    pub(crate) file_name: String,
    pub(crate) options: CheckerOptions,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) symbols: SymbolTable,
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) ambient_modules: std::collections::HashMap<String, ModuleExportTable>,
    pub(crate) ambient_global_symbols: SymbolTable,
    pub(crate) ambient_global_type_declarations: TypeDeclarationTable,
}

impl CheckerContext {
    pub(crate) fn new(file_name: String, options: CheckerOptions) -> Self {
        Self {
            file_name,
            options,
            diagnostics: Vec::new(),
            symbols: SymbolTable::new(),
            type_declarations: TypeDeclarationTable::new(),
            ambient_modules: std::collections::HashMap::new(),
            ambient_global_symbols: SymbolTable::new(),
            ambient_global_type_declarations: TypeDeclarationTable::new(),
        }
    }

    pub(crate) fn set_file_name(&mut self, file_name: String) {
        self.file_name = file_name;
    }

    pub(crate) fn set_symbols(&mut self, symbols: SymbolTable) {
        self.symbols = symbols;
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
