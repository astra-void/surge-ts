use std::collections::HashMap;
use std::sync::Arc;

use surge_ts_syntax::{ParsedExpression, ParsedFunctionBodyStatement, TextSpan as SyntaxTextSpan};

use crate::context::CheckerContext;
use crate::program::{
    record_flow_branch_changed_local_count, record_flow_branch_empty_delta_count,
    record_flow_read_lookup_count, record_flow_scope_pop_count, record_flow_scope_push_count,
    record_flow_state_clone_count, record_flow_state_full_clone_avoided_count,
};

mod branch;
mod expr;
mod facts;

pub(crate) use branch::*;
pub(crate) use expr::*;
pub(crate) use facts::*;

pub(crate) fn check_expression_flow(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
    check_expression_flow_impl(expression, fallback_span, flow_state, statement_index, ctx)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionBodyFlow {
    pub(crate) contains_value_return: bool,
    pub(crate) guarantees_value_return: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FunctionFlowFacts {
    pub(crate) has_let_or_const: bool,
    pub(crate) has_uninitialized_let_or_const: bool,
    pub(crate) has_assignments: bool,
    pub(crate) has_identifier_reads: bool,
    pub(crate) has_future_block_scoped_declarations: bool,
    pub(crate) has_return_or_throw: bool,
    pub(crate) has_branching: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssignmentState {
    DeclaredUnassigned,
    Assigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowCheck {
    Clear,
    Blocked,
}

impl FlowCheck {
    pub(crate) fn is_blocked(self) -> bool {
        matches!(self, Self::Blocked)
    }
}

#[derive(Debug)]
pub(crate) struct FunctionFlowState {
    enabled: bool,
    tracked_local_count: usize,
    scopes: Vec<FlowScope>,
    branch_captures: Vec<FlowBranchCapture>,
}

impl Clone for FunctionFlowState {
    fn clone(&self) -> Self {
        record_flow_state_clone_count(
            self.scopes
                .iter()
                .map(|scope| scope.locals.len())
                .sum::<usize>(),
        );

        Self {
            enabled: self.enabled,
            tracked_local_count: self.tracked_local_count,
            scopes: self.scopes.clone(),
            branch_captures: self.branch_captures.clone(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FlowBranchDelta {
    changes: HashMap<usize, HashMap<Arc<str>, AssignmentState>>,
    changed_local_count: usize,
    pub(crate) continues: bool,
}

impl FlowBranchDelta {
    fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    fn get(&self, scope_index: usize, name: &str) -> Option<AssignmentState> {
        self.changes
            .get(&scope_index)
            .and_then(|scope_changes| scope_changes.get(name))
            .copied()
    }
}

#[derive(Debug, Default, Clone)]
struct FlowBranchCapture {
    scope_count: usize,
    tracked_local_count: usize,
    previous_states: HashMap<usize, HashMap<Arc<str>, Option<AssignmentState>>>,
}

#[derive(Debug, Default)]
struct FlowScope {
    locals: HashMap<Arc<str>, AssignmentState>,
    future_block_scoped_declarations: HashMap<Arc<str>, usize>,
    tracked_count: usize,
}

impl Clone for FlowScope {
    fn clone(&self) -> Self {
        Self {
            locals: self.locals.clone(),
            future_block_scoped_declarations: self.future_block_scoped_declarations.clone(),
            tracked_count: self.tracked_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowReadOutcome {
    Unresolved,
    UseBeforeDeclaration,
    Declared(AssignmentState),
}

pub(crate) fn analyze_function_body_flow(body: &[ParsedFunctionBodyStatement]) -> FunctionBodyFlow {
    let summary = summarize_function_body_flow(body);
    FunctionBodyFlow {
        contains_value_return: summary.contains_value_return,
        guarantees_value_return: summary.guarantees_value_return,
    }
}

pub(crate) fn collect_function_flow_facts(
    body: &[ParsedFunctionBodyStatement],
) -> FunctionFlowFacts {
    let mut facts = FunctionFlowFacts::default();
    collect_function_flow_facts_from_body(body, &mut facts);
    facts
}

pub(crate) fn mark_assignment_state(target_name: &str, flow_state: &mut FunctionFlowState) {
    flow_state.mark_assigned(target_name);
}

impl FunctionFlowState {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            tracked_local_count: 0,
            scopes: Vec::new(),
            branch_captures: Vec::new(),
        }
    }

    pub(crate) fn tracked_local_count(&self) -> usize {
        self.tracked_local_count
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn begin_branch_capture(&mut self) {
        if !self.enabled {
            return;
        }

        record_flow_state_full_clone_avoided_count();
        self.branch_captures.push(FlowBranchCapture {
            scope_count: self.scopes.len(),
            tracked_local_count: self.tracked_local_count,
            previous_states: HashMap::new(),
        });
    }

    pub(crate) fn finish_branch_capture(&mut self) -> FlowBranchDelta {
        if !self.enabled {
            return FlowBranchDelta::default();
        }

        let Some(capture) = self.branch_captures.pop() else {
            return FlowBranchDelta::default();
        };

        let mut delta = FlowBranchDelta::default();

        for (scope_index, scope_changes) in &capture.previous_states {
            if *scope_index >= capture.scope_count {
                continue;
            }

            let Some(scope) = self.scopes.get(*scope_index) else {
                continue;
            };

            for name in scope_changes.keys() {
                if let Some(current_state) = scope.locals.get(name).copied() {
                    delta
                        .changes
                        .entry(*scope_index)
                        .or_default()
                        .insert(name.clone(), current_state);
                    delta.changed_local_count += 1;
                }
            }
        }

        if delta.is_empty() {
            record_flow_branch_empty_delta_count();
        } else {
            record_flow_branch_changed_local_count(delta.changed_local_count);
        }

        self.restore_branch_capture(capture);
        delta
    }

    pub(crate) fn push_scope(
        &mut self,
        future_block_scoped_declarations: HashMap<Arc<str>, usize>,
    ) {
        if !self.enabled {
            return;
        }

        let tracked_count = future_block_scoped_declarations.len();
        self.tracked_local_count += tracked_count;
        record_flow_scope_push_count();
        self.scopes.push(FlowScope {
            locals: HashMap::new(),
            future_block_scoped_declarations,
            tracked_count,
        });
    }

    pub(crate) fn pop_scope(&mut self) {
        if !self.enabled {
            return;
        }

        if let Some(scope) = self.scopes.pop() {
            self.tracked_local_count = self.tracked_local_count.saturating_sub(scope.tracked_count);
            record_flow_scope_pop_count();
        }
    }

    pub(crate) fn declare_current(&mut self, name: impl Into<Arc<str>>, state: AssignmentState) {
        if !self.enabled {
            return;
        }

        let name = name.into();
        let scope_index = self
            .scopes
            .len()
            .checked_sub(1)
            .expect("flow state must contain at least one scope");
        let scope = self
            .scopes
            .last_mut()
            .expect("flow state must contain at least one scope");
        let previous_state = scope.locals.get(&name).copied();
        if previous_state == Some(state) {
            return;
        }

        let record_name = self
            .branch_captures
            .last()
            .is_some_and(|capture| scope_index < capture.scope_count)
            .then(|| name.clone());

        scope.locals.insert(name, state);

        if let Some(capture) = self.branch_captures.last_mut() {
            if scope_index < capture.scope_count {
                Self::record_branch_change(
                    capture,
                    scope_index,
                    record_name.expect("branch capture name should be recorded"),
                    previous_state,
                );
            }
        }

        if previous_state.is_none() {
            scope.tracked_count += 1;
            self.tracked_local_count += 1;
        }
    }

    pub(crate) fn mark_assigned(&mut self, name: &str) {
        if !self.enabled || self.tracked_local_count == 0 {
            return;
        }

        let mut found_scope_index = None;
        for (scope_index, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(current_state) = scope.locals.get(name).copied() {
                if current_state == AssignmentState::Assigned {
                    return;
                }
                found_scope_index = Some(scope_index);
                break;
            }
        }

        let Some(scope_index) = found_scope_index else {
            return;
        };

        let owned_name: Arc<str> = name.into();
        let record_name = self
            .branch_captures
            .last()
            .is_some_and(|capture| scope_index < capture.scope_count)
            .then(|| owned_name.clone());

        if let Some(scope) = self.scopes.get_mut(scope_index) {
            let previous_state = scope.locals.insert(owned_name, AssignmentState::Assigned);
            if let Some(capture) = self.branch_captures.last_mut() {
                if scope_index < capture.scope_count {
                    Self::record_branch_change(
                        capture,
                        scope_index,
                        record_name.expect("branch capture name should be recorded"),
                        previous_state,
                    );
                }
            }
        }
    }

    fn restore_branch_capture(&mut self, capture: FlowBranchCapture) {
        self.restore_scope_count(capture.scope_count);

        for (scope_index, scope_changes) in capture.previous_states {
            if scope_index >= self.scopes.len() {
                continue;
            }

            let Some(scope) = self.scopes.get_mut(scope_index) else {
                continue;
            };

            for (name, previous_state) in scope_changes {
                match previous_state {
                    Some(previous_state) => {
                        scope.locals.insert(name, previous_state);
                    }
                    None => {
                        if scope.locals.remove(&name).is_some() {
                            scope.tracked_count = scope.tracked_count.saturating_sub(1);
                        }
                    }
                }
            }
        }

        self.tracked_local_count = capture.tracked_local_count;
    }

    fn restore_scope_count(&mut self, scope_count: usize) {
        while self.scopes.len() > scope_count {
            let Some(scope) = self.scopes.pop() else {
                break;
            };
            self.tracked_local_count = self.tracked_local_count.saturating_sub(scope.tracked_count);
        }
    }

    fn record_branch_change(
        capture: &mut FlowBranchCapture,
        scope_index: usize,
        name: Arc<str>,
        previous_state: Option<AssignmentState>,
    ) {
        capture
            .previous_states
            .entry(scope_index)
            .or_default()
            .entry(name)
            .or_insert(previous_state);
    }

    fn read_identifier(&self, name: &str, statement_index: usize) -> FlowReadOutcome {
        if !self.enabled || self.tracked_local_count == 0 {
            return FlowReadOutcome::Unresolved;
        }

        let Some(current_scope) = self.scopes.last() else {
            return FlowReadOutcome::Unresolved;
        };

        let mut lookup_steps = 1usize;

        if let Some(state) = current_scope.locals.get(name).copied() {
            record_flow_read_lookup_count(lookup_steps);
            return FlowReadOutcome::Declared(state);
        }

        if current_scope
            .future_block_scoped_declarations
            .get(name)
            .is_some_and(|declaration_index| statement_index < *declaration_index)
        {
            record_flow_read_lookup_count(lookup_steps);
            return FlowReadOutcome::UseBeforeDeclaration;
        }

        for scope in self.scopes.iter().rev().skip(1) {
            lookup_steps += 1;
            if let Some(state) = scope.locals.get(name).copied() {
                record_flow_read_lookup_count(lookup_steps);
                return FlowReadOutcome::Declared(state);
            }
        }

        record_flow_read_lookup_count(lookup_steps);
        FlowReadOutcome::Unresolved
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ReturnFlowSummary {
    contains_value_return: bool,
    contains_throw: bool,
    guarantees_value_return: bool,
}
