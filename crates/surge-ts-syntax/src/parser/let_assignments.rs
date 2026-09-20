//! Where each `let` tsc may type by control flow is assigned — tsc's
//! `markNodeAssignments` over the declaring container. A closure reading such
//! a binding continues the enclosing flow only past its last assignment
//! (`isPastLastAssignment`), and one never definitely assigned reads as
//! uninitialized (`isSymbolAssignedDefinitely`). Only an un-annotated `let`
//! with no initializer or an `undefined`/`null`/`[]` one can be such a binding,
//! so files without one skip the walk.

use std::collections::HashMap;

use oxc_ast::ast::{
    AssignmentExpression, AssignmentOperator, AssignmentTarget, BindingPattern, Class, Expression,
    ForInStatement, ForOfStatement, FormalParameters, Function, Program,
    SimpleAssignmentTarget, Statement, UpdateExpression, VariableDeclaration,
    VariableDeclarationKind,
};
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;
use oxc_syntax::scope::{ScopeFlags, ScopeId};

use crate::LetAssignmentSummary;

pub(crate) fn collect_let_assignments(
    program: &Program<'_>,
    source_text: &str,
) -> Vec<LetAssignmentSummary> {
    if !has_candidate_declaration(source_text, "let") {
        return Vec::new();
    }
    let mut collector = Collector {
        is_module: program.source_type.is_module(),
        ..Collector::default()
    };
    collector.visit_program(program);
    collector.resolve()
}

/// Cheap pre-filter: `let` followed by a name and then `;`, `,`, a line end,
/// or `= undefined` / `= null` / `= []`.
fn has_candidate_declaration(source_text: &str, keyword: &str) -> bool {
    let bytes = source_text.as_bytes();
    let mut from = 0;
    while let Some(offset) = source_text[from..].find(keyword) {
        let start = from + offset;
        from = start + keyword.len();
        if start > 0 && is_identifier_byte(bytes[start - 1]) {
            continue;
        }
        let mut index = start + keyword.len();
        if index >= bytes.len() || !bytes[index].is_ascii_whitespace() {
            continue;
        }
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len() && is_identifier_byte(bytes[index]) {
            index += 1;
        }
        if index == name_start {
            continue;
        }
        while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
            index += 1;
        }
        let is_let = keyword == "let";
        match bytes.get(index) {
            None | Some(b';' | b',' | b'\n' | b'\r') if is_let => return true,
            Some(b'=') => {
                let rest = source_text[index + 1..].trim_start();
                if rest.starts_with("[]")
                    || is_let && (rest.starts_with("undefined") || rest.starts_with("null"))
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

struct Scope {
    parent: Option<usize>,
    /// The innermost function (or the file) this scope belongs to.
    function: usize,
    names: HashMap<String, Option<usize>>,
}

struct Candidate {
    name_start: u32,
    declaration_start: u32,
    function: usize,
    last_assignment: Option<u32>,
    definitely_assigned: bool,
}

struct Write {
    name: String,
    scope: usize,
    function: usize,
    position: u32,
    definite: bool,
    /// The statements enclosing the write that `extendAssignmentPosition`
    /// extends through, innermost last.
    statements: Vec<(u32, u32)>,
}

#[derive(Default)]
struct Collector {
    is_module: bool,
    scopes: Vec<Scope>,
    stack: Vec<usize>,
    candidates: Vec<Candidate>,
    writes: Vec<Write>,
    statements: Vec<(u32, u32)>,
    exported_depth: usize,
}

impl Collector {}

impl Collector {
    fn current(&self) -> usize {
        *self.stack.last().expect("inside the program scope")
    }

    fn declare(&mut self, scope: usize, name: &str, candidate: Option<usize>) {
        self.scopes[scope].names.insert(name.to_string(), candidate);
    }

    fn resolve_name(&self, name: &str, scope: usize) -> Option<usize> {
        let mut scope = Some(scope);
        while let Some(current) = scope {
            if let Some(found) = self.scopes[current].names.get(name) {
                return *found;
            }
            scope = self.scopes[current].parent;
        }
        None
    }

    fn declare_pattern(&mut self, scope: usize, pattern: &BindingPattern<'_>) {
        let mut names = Vec::new();
        collect_binding_names(pattern, &mut names);
        for name in names {
            self.declare(scope, &name, None);
        }
    }

    fn function_scope(&self) -> usize {
        self.scopes[self.current()].function
    }

    fn record_write(&mut self, name: &str, position: u32, definite: bool) {
        let scope = self.current();
        self.writes.push(Write {
            name: name.to_string(),
            scope,
            function: self.scopes[scope].function,
            position,
            definite,
            statements: self.statements.clone(),
        });
    }

    fn record_target(&mut self, target: &AssignmentTarget<'_>, definite: bool) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                self.record_write(&identifier.name, identifier.span.start, definite);
            }
            AssignmentTarget::ArrayAssignmentTarget(_) | AssignmentTarget::ObjectAssignmentTarget(_) => {
                let mut names = Vec::new();
                collect_target_names(target, &mut names);
                for (name, position) in names {
                    self.record_write(&name, position, definite);
                }
            }
            _ => {}
        }
    }

    fn resolve(mut self) -> Vec<LetAssignmentSummary> {
        for write in std::mem::take(&mut self.writes) {
            let Some(index) = self.resolve_name(&write.name, write.scope) else {
                continue;
            };
            let candidate = &mut self.candidates[index];
            if write.definite {
                candidate.definitely_assigned = true;
            }
            if candidate.last_assignment == Some(u32::MAX) {
                continue;
            }
            let position = if write.function == candidate.function {
                let mut position = write.position;
                for &(start, end) in write.statements.iter().rev() {
                    if start <= candidate.declaration_start {
                        break;
                    }
                    position = end;
                }
                position
            } else {
                u32::MAX
            };
            candidate.last_assignment =
                Some(candidate.last_assignment.map_or(position, |last| last.max(position)));
        }
        let mut summaries: Vec<LetAssignmentSummary> = self
            .candidates
            .into_iter()
            .map(|candidate| LetAssignmentSummary {
                name_start: candidate.name_start,
                last_assignment: candidate.last_assignment,
                definitely_assigned: candidate.definitely_assigned,
            })
            .collect();
        summaries.sort_unstable_by_key(|summary| summary.name_start);
        summaries
    }
}

fn collect_binding_names(pattern: &BindingPattern<'_>, names: &mut Vec<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => names.push(identifier.name.to_string()),
        BindingPattern::AssignmentPattern(assignment) => collect_binding_names(&assignment.left, names),
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_binding_names(&property.value, names);
            }
            if let Some(rest) = &object.rest {
                collect_binding_names(&rest.argument, names);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_binding_names(element, names);
            }
            if let Some(rest) = &array.rest {
                collect_binding_names(&rest.argument, names);
            }
        }
    }
}

fn collect_target_names(target: &AssignmentTarget<'_>, names: &mut Vec<(String, u32)>) {
    #[derive(Default)]
    struct Names(Vec<(String, u32)>);
    impl<'a> Visit<'a> for Names {
        fn visit_identifier_reference(&mut self, reference: &oxc_ast::ast::IdentifierReference<'a>) {
            self.0.push((reference.name.to_string(), reference.span.start));
        }
        // A member target writes the member; its object is only read.
        fn visit_member_expression(&mut self, _: &oxc_ast::ast::MemberExpression<'a>) {}
        // A default value is read, not written.
        fn visit_expression(&mut self, _: &Expression<'a>) {}
    }
    let mut collected = Names::default();
    collected.visit_assignment_target(target);
    names.extend(collected.0);
}

fn is_candidate_initializer(init: Option<&Expression<'_>>) -> bool {
    match init.map(Expression::without_parentheses) {
        None => true,
        Some(Expression::Identifier(identifier)) => identifier.name == "undefined",
        Some(Expression::NullLiteral(_)) => true,
        Some(Expression::ArrayExpression(array)) => array.elements.is_empty(),
        _ => false,
    }
}

/// The statements tsc's `extendAssignmentPosition` extends a write through.
fn extends_assignment_position(statement: &Statement<'_>) -> bool {
    matches!(
        statement,
        Statement::VariableDeclaration(_)
            | Statement::ExpressionStatement(_)
            | Statement::IfStatement(_)
            | Statement::DoWhileStatement(_)
            | Statement::WhileStatement(_)
            | Statement::ForStatement(_)
            | Statement::ForInStatement(_)
            | Statement::ForOfStatement(_)
            | Statement::WithStatement(_)
            | Statement::SwitchStatement(_)
            | Statement::TryStatement(_)
            | Statement::ClassDeclaration(_)
    )
}

impl<'a> Visit<'a> for Collector {
    fn enter_scope(&mut self, flags: ScopeFlags, _: &std::cell::Cell<Option<ScopeId>>) {
        let index = self.scopes.len();
        let parent = self.stack.last().copied();
        let function = if flags.is_function() || flags.is_top() || parent.is_none() {
            index
        } else {
            self.scopes[parent.expect("checked above")].function
        };
        self.scopes.push(Scope {
            parent,
            function,
            names: HashMap::new(),
        });
        self.stack.push(index);
    }

    fn leave_scope(&mut self) {
        self.stack.pop();
    }

    fn visit_statement(&mut self, statement: &Statement<'a>) {
        let extends = extends_assignment_position(statement);
        if extends {
            let span = statement.span();
            self.statements.push((span.start, span.end));
        }
        oxc_ast_visit::walk::walk_statement(self, statement);
        if extends {
            self.statements.pop();
        }
    }

    fn visit_export_named_declaration(
        &mut self,
        declaration: &oxc_ast::ast::ExportNamedDeclaration<'a>,
    ) {
        self.exported_depth += 1;
        oxc_ast_visit::walk::walk_export_named_declaration(self, declaration);
        self.exported_depth -= 1;
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
        let scope = match declaration.kind {
            VariableDeclarationKind::Var => self.function_scope(),
            _ => self.current(),
        };
        let top_level_script = !self.is_module && scope == 0;
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                self.declare_pattern(scope, &declarator.id);
                continue;
            };
            let flow_typed = !declaration.declare
                && declarator.type_annotation.is_none()
                && self.exported_depth == 0;
            let mut candidate = None;
            if flow_typed
                && declaration.kind == VariableDeclarationKind::Let
                && !top_level_script
                && is_candidate_initializer(declarator.init.as_ref())
            {
                candidate = Some(self.candidates.len());
                self.candidates.push(Candidate {
                    name_start: identifier.span.start,
                    declaration_start: declarator.span.start,
                    function: self.scopes[scope].function,
                    last_assignment: None,
                    definitely_assigned: false,
                });
            }
            self.declare(scope, &identifier.name, candidate);
        }
        oxc_ast_visit::walk::walk_variable_declaration(self, declaration);
    }

    fn visit_formal_parameters(&mut self, parameters: &FormalParameters<'a>) {
        let scope = self.current();
        for parameter in &parameters.items {
            self.declare_pattern(scope, &parameter.pattern);
        }
        if let Some(rest) = &parameters.rest {
            self.declare_pattern(scope, &rest.rest.argument);
        }
        oxc_ast_visit::walk::walk_formal_parameters(self, parameters);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &function.id
            && function.is_declaration()
        {
            let scope = self.current();
            self.declare(scope, &id.name, None);
        }
        oxc_ast_visit::walk::walk_function(self, function, flags);
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        if let Some(id) = &class.id
            && class.is_declaration()
        {
            let scope = self.current();
            self.declare(scope, &id.name, None);
        }
        oxc_ast_visit::walk::walk_class(self, class);
    }

    fn visit_catch_parameter(&mut self, parameter: &oxc_ast::ast::CatchParameter<'a>) {
        let scope = self.current();
        self.declare_pattern(scope, &parameter.pattern);
        oxc_ast_visit::walk::walk_catch_parameter(self, parameter);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        let definite = matches!(
            assignment.operator,
            AssignmentOperator::Assign
                | AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalNullish
        );
        self.record_target(&assignment.left, definite);
        oxc_ast_visit::walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument {
            self.record_write(&identifier.name, identifier.span.start, false);
        }
        oxc_ast_visit::walk::walk_update_expression(self, update);
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        if let Some(target) = statement.left.as_assignment_target() {
            self.record_target(target, true);
        }
        oxc_ast_visit::walk::walk_for_in_statement(self, statement);
    }

    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        if let Some(target) = statement.left.as_assignment_target() {
            self.record_target(target, true);
        }
        oxc_ast_visit::walk::walk_for_of_statement(self, statement);
    }
}
