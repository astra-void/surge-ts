use std::collections::{HashMap, HashSet};

use typescript_rust_diagnostics::{Diagnostic, TextSpan as DiagnosticTextSpan};
use typescript_rust_syntax::TextSpan as SyntaxTextSpan;

use crate::symbols::{SymbolTable, TypeDeclarationTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticProfile {
    #[default]
    Tsc,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    RootSource,
    RootDeclaration,
    DependencyDeclaration,
    GeneratedDeclaration,
}

impl FileKind {
    pub(crate) fn is_declaration(self) -> bool {
        matches!(
            self,
            FileKind::RootDeclaration
                | FileKind::DependencyDeclaration
                | FileKind::GeneratedDeclaration
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompatibilityStats {
    pub suppressed_diagnostics_total: usize,
    pub suppressed_declaration_diagnostics_total: usize,
    pub suppressed_rust_only_diagnostics_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckerOptions {
    pub no_implicit_any: bool,
    pub stub_external_modules: bool,
    pub resolved_modules: std::collections::HashMap<String, String>,
    pub no_lib: bool,
    pub skip_lib_check: bool,
    pub diagnostic_profile: DiagnosticProfile,
}

impl Default for CheckerOptions {
    fn default() -> Self {
        Self {
            no_implicit_any: false,
            stub_external_modules: false,
            resolved_modules: std::collections::HashMap::new(),
            no_lib: false,
            skip_lib_check: false,
            diagnostic_profile: DiagnosticProfile::default(),
        }
    }
}

use crate::modules::ModuleExportTable;

#[derive(Debug, Clone)]
pub(crate) struct CheckerContext {
    pub(crate) file_name: String,
    pub(crate) current_file_kind: FileKind,
    pub(crate) options: CheckerOptions,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) stats: CompatibilityStats,
    pub(crate) utility_diagnostic_keys: HashSet<UtilityDiagnosticKey>,
    pub(crate) symbols: SymbolTable,
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) ambient_modules: std::collections::HashMap<String, ModuleExportTable>,
    pub(crate) ambient_global_symbols: SymbolTable,
    pub(crate) ambient_global_type_declarations: TypeDeclarationTable,
    file_kinds: HashMap<String, FileKind>,
}

impl CheckerContext {
    pub(crate) fn new(
        file_name: String,
        options: CheckerOptions,
        file_kinds: HashMap<String, FileKind>,
    ) -> Self {
        let current_file_kind = file_kinds
            .get(&file_name)
            .copied()
            .unwrap_or(FileKind::RootSource);

        Self {
            file_name,
            current_file_kind,
            options,
            diagnostics: Vec::new(),
            stats: CompatibilityStats::default(),
            utility_diagnostic_keys: HashSet::new(),
            symbols: SymbolTable::new(),
            type_declarations: TypeDeclarationTable::new(),
            ambient_modules: std::collections::HashMap::new(),
            ambient_global_symbols: SymbolTable::new(),
            ambient_global_type_declarations: TypeDeclarationTable::new(),
            file_kinds,
        }
    }

    pub(crate) fn set_file_name(&mut self, file_name: String) {
        self.current_file_kind = self
            .file_kinds
            .get(&file_name)
            .copied()
            .unwrap_or(FileKind::RootSource);
        self.file_name = file_name;
    }

    pub(crate) fn set_symbols(&mut self, symbols: SymbolTable) {
        self.symbols = symbols;
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        if self.should_suppress(&diagnostic) {
            self.record_suppressed(&diagnostic);
            return;
        }

        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn push_utility_diagnostic_once(&mut self, diagnostic: Diagnostic) {
        let key = UtilityDiagnosticKey {
            code: diagnostic.code.to_string(),
            file_name: diagnostic.file_name.clone(),
            span: diagnostic.span.map(|span| (span.start, span.end)),
        };

        if self.utility_diagnostic_keys.insert(key) {
            self.push(diagnostic);
        }
    }

    pub(crate) fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(crate) fn truncate_diagnostics(&mut self, len: usize) {
        self.diagnostics.truncate(len);
    }

    pub(crate) fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    pub(crate) fn finish_with_stats(self) -> (Vec<Diagnostic>, CompatibilityStats) {
        (self.diagnostics, self.stats)
    }

    fn should_suppress(&self, diagnostic: &Diagnostic) -> bool {
        if self.options.diagnostic_profile == DiagnosticProfile::Native {
            return false;
        }

        let code = diagnostic.code.to_string();
        if code.starts_with("typescript-rust::") {
            return true;
        }

        if self.options.skip_lib_check && self.current_file_kind.is_declaration() {
            return true;
        }

        false
    }

    fn record_suppressed(&mut self, diagnostic: &Diagnostic) {
        self.stats.suppressed_diagnostics_total += 1;

        if self.current_file_kind.is_declaration() {
            self.stats.suppressed_declaration_diagnostics_total += 1;
        }

        if is_rust_only_compat_diagnostic(&diagnostic.code.to_string()) {
            self.stats.suppressed_rust_only_diagnostics_total += 1;
        }
    }
}

pub(crate) fn convert_span(span: SyntaxTextSpan) -> DiagnosticTextSpan {
    DiagnosticTextSpan {
        start: span.start,
        end: span.end,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UtilityDiagnosticKey {
    code: String,
    file_name: String,
    span: Option<(usize, usize)>,
}

fn is_rust_only_compat_diagnostic(code: &str) -> bool {
    code.starts_with("typescript-rust::")
}
