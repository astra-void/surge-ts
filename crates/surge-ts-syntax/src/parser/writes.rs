//! Every name a file writes by *definite* assignment — tsc's
//! `AssignmentKindDefinite`: `x = v`, `x ??= v`/`||=`/`&&=`, a destructuring
//! target, and a bare `for (x of …)`/`for (x in …)` head. Compound assignment
//! and `x++` are not definite. Collected over the full oxc AST, nested
//! functions included, because `isSymbolAssignedDefinitely` asks whether such
//! a write exists *anywhere* in the declaring container. Every identifier inside
//! a target counts except inside a member target (`o` in `o.p = v` is read, not
//! written); a destructuring default's names count too, which only withholds a
//! TS2454, never adds one.

use std::collections::HashSet;

use oxc_ast::ast::{
    AssignmentExpression, AssignmentOperator, ForInStatement, ForOfStatement, IdentifierReference,
    MemberExpression, Program,
};
use oxc_ast_visit::Visit;

#[derive(Default)]
struct TargetNames<'a> {
    names: HashSet<&'a str>,
}

impl<'a> Visit<'a> for TargetNames<'a> {
    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        self.names.insert(reference.name.as_str());
    }

    // `o.p = v` writes the member, not `o`, which stays as unassigned as it was.
    fn visit_member_expression(&mut self, _member: &MemberExpression<'a>) {}
}

#[derive(Default)]
struct WriteCollector<'a> {
    names: HashSet<&'a str>,
}

impl<'a> Visit<'a> for WriteCollector<'a> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if matches!(
            assignment.operator,
            AssignmentOperator::Assign
                | AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalNullish
        ) {
            let mut targets = TargetNames::default();
            targets.visit_assignment_target(&assignment.left);
            self.names.extend(targets.names);
        }
        oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        let mut targets = TargetNames::default();
        targets.visit_for_statement_left(&statement.left);
        self.names.extend(targets.names);
        oxc_ast_visit::walk::walk_for_in_statement(self, statement);
    }

    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        let mut targets = TargetNames::default();
        targets.visit_for_statement_left(&statement.left);
        self.names.extend(targets.names);
        oxc_ast_visit::walk::walk_for_of_statement(self, statement);
    }
}

pub(super) fn collect_definite_writes(program: &Program<'_>) -> Vec<String> {
    let mut collector = WriteCollector::default();
    collector.visit_program(program);
    let mut names: Vec<String> = collector.names.into_iter().map(str::to_string).collect();
    names.sort_unstable();
    names
}
