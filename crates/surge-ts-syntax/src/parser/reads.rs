//! Collects every value-position identifier name read within a function body,
//! walking the full oxc AST. Because it runs over the original AST (not the
//! lossy `Parsed*` tree), it sees reads inside spreads, `for-in` loops, object
//! methods, and nested functions — the over-approximation that backs FP-free
//! unused-binding diagnostics (TS6133).

use oxc_ast::ast::{FunctionBody, IdentifierReference, Program};
use oxc_ast_visit::Visit;

#[derive(Default)]
struct ReadCollector {
    names: Vec<String>,
}

impl<'a> Visit<'a> for ReadCollector {
    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        self.names.push(reference.name.to_string());
    }
}

fn finish(mut collector: ReadCollector) -> Vec<String> {
    collector.names.sort_unstable();
    collector.names.dedup();
    collector.names
}

pub(crate) fn collect_function_body_reads(body: &FunctionBody<'_>) -> Vec<String> {
    let mut collector = ReadCollector::default();
    collector.visit_function_body(body);
    finish(collector)
}

pub(crate) fn collect_program_reads(program: &Program<'_>) -> Vec<String> {
    let mut collector = ReadCollector::default();
    collector.visit_program(program);
    finish(collector)
}
