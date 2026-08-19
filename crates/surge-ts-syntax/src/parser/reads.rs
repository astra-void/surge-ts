//! Collects every identifier name read within a function body — value positions
//! and type positions alike — walking the full oxc AST. Because it runs over the
//! original AST (not the lossy `Parsed*` tree), it sees reads inside spreads,
//! `for-in` loops, object methods, nested functions, and type annotations — the
//! over-approximation that backs FP-free unused-binding diagnostics (TS6133,
//! TS6196).

use oxc_ast::ast::{
    AssignmentExpression, AssignmentOperator, AssignmentTarget, FunctionBody, IdentifierReference,
    Program, TSTypeName,
};
use oxc_ast_visit::Visit;

#[derive(Default)]
struct ReadCollector {
    names: Vec<String>,
}

impl<'a> Visit<'a> for ReadCollector {
    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        self.names.push(reference.name.to_string());
    }

    /// A plain `x = value` *writes* `x`; it does not read it, so a binding that is
    /// only ever assigned stays unused (tsc reports TS6133 for it). Compound
    /// forms (`x += 1`) and every member target (`o.p = v` reads `o`) keep the
    /// default walk.
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if assignment.operator == AssignmentOperator::Assign
            && matches!(
                assignment.left,
                AssignmentTarget::AssignmentTargetIdentifier(_)
            )
        {
            self.visit_expression(&assignment.right);
            return;
        }
        oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
    }

    /// Type-position names count as reads too: they are what makes a body-local
    /// `type`/`interface` used (TS6196), and a value named in `typeof x` is read
    /// by tsc even though it never appears in value position.
    fn visit_ts_type_name(&mut self, name: &TSTypeName<'a>) {
        let mut current = name;
        loop {
            match current {
                TSTypeName::IdentifierReference(identifier) => {
                    self.names.push(identifier.name.to_string());
                    return;
                }
                TSTypeName::QualifiedName(qualified) => current = &qualified.left,
                TSTypeName::ThisExpression(_) => return,
            }
        }
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
