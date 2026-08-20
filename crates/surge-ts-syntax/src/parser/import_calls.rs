//! Module specifiers written as `import("...")` rather than as an
//! `import`/`export` declaration: type-position import types
//! (`import("pkg").T`, `typeof import("pkg")`) and dynamic import expressions.
//!
//! tsc puts both in the program's module graph, so the loader has to resolve
//! them too — a package reached only through an import type still contributes
//! its `/// <reference types="..." />` directives and its ambient
//! `declare module` blocks.

use oxc_ast::ast::{Expression, ImportExpression, Program, TSImportType};
use oxc_ast_visit::Visit;

#[derive(Default)]
struct ImportCallCollector {
    specifiers: Vec<String>,
}

impl<'a> Visit<'a> for ImportCallCollector {
    fn visit_ts_import_type(&mut self, import_type: &TSImportType<'a>) {
        self.specifiers.push(import_type.source.value.to_string());
        oxc_ast_visit::walk::walk_ts_import_type(self, import_type);
    }

    fn visit_import_expression(&mut self, import_expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(literal) = &import_expression.source {
            self.specifiers.push(literal.value.to_string());
        }
        oxc_ast_visit::walk::walk_import_expression(self, import_expression);
    }
}

/// Cheap pre-filter: an `import` keyword immediately followed by `(` (modulo
/// whitespace). Skips the whole-AST walk for the vast majority of files, which
/// never use the form.
fn has_import_call(source_text: &str) -> bool {
    let bytes = source_text.as_bytes();
    let mut rest = source_text;
    let mut base = 0usize;
    while let Some(offset) = rest.find("import") {
        let mut index = base + offset + "import".len();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) == Some(&b'(') {
            return true;
        }
        base += offset + "import".len();
        rest = &source_text[base..];
    }
    false
}

pub(crate) fn collect_import_call_specifiers(
    program: &Program<'_>,
    source_text: &str,
) -> Vec<String> {
    if !has_import_call(source_text) {
        return Vec::new();
    }
    let mut collector = ImportCallCollector::default();
    collector.visit_program(program);

    // Bundled `.d.ts` files repeat the same `import("pkg")` on dozens of
    // members; the loader resolves each specifier per importer, so collapse
    // them here rather than in every consumer.
    let mut seen = std::collections::HashSet::new();
    collector
        .specifiers
        .retain(|specifier| seen.insert(specifier.clone()));
    collector.specifiers
}
