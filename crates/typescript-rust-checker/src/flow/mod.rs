use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedExpression, ParsedFunctionBodyStatement, ParsedVariableKind, TextSpan as SyntaxTextSpan,
};

use crate::context::{CheckerContext, convert_span};
use crate::program::{
    record_flow_branch_changed_local_count, record_flow_branch_empty_delta_count,
    record_flow_branch_merge_count, record_flow_branch_merge_fast_path_count,
    record_flow_branch_merge_local_iteration_count, record_flow_expression_visit_count,
    record_flow_future_declaration_collection_count, record_flow_identifier_read_count,
    record_flow_read_lookup_count, record_flow_return_analysis_walk_count,
    record_flow_scope_pop_count, record_flow_scope_push_count, record_flow_state_clone_count,
    record_flow_state_full_clone_avoided_count, record_flow_truthiness_check_count,
};

thread_local! {
    static EMIT_USE_BEFORE_DECLARATION_AS_UNASSIGNED: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn with_use_before_declaration_as_unassigned<R>(
    enabled: bool,
    f: impl FnOnce() -> R,
) -> R {
    EMIT_USE_BEFORE_DECLARATION_AS_UNASSIGNED.with(|flag| {
        let previous = flag.replace(enabled);
        let result = f();
        flag.set(previous);
        result
    })
}

pub(crate) fn check_expression_flow(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
    check_expression_flow_impl(expression, fallback_span, flow_state, statement_index, ctx)
}

fn should_emit_use_before_declaration_as_unassigned() -> bool {
    EMIT_USE_BEFORE_DECLARATION_AS_UNASSIGNED.with(Cell::get)
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

pub(crate) fn collect_future_block_scoped_declarations(
    body: &[ParsedFunctionBodyStatement],
) -> HashMap<Arc<str>, usize> {
    let mut declarations = HashMap::new();

    for (index, statement) in body.iter().enumerate() {
        if let ParsedFunctionBodyStatement::VariableDeclaration(variable) = statement {
            if matches!(
                variable.kind,
                ParsedVariableKind::Let | ParsedVariableKind::Const
            ) {
                declarations
                    .entry(variable.name.clone().into())
                    .or_insert(index);
            }
        }
    }

    record_flow_future_declaration_collection_count(declarations.len());
    declarations
}

pub(crate) fn check_obvious_truthiness_condition(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    record_flow_truthiness_check_count();
    // This is intentionally narrow: it only covers syntax the project already parses
    // and only emits the obvious truthiness diagnostics that the current checker supports.
    let (diagnostic, diagnostic_emitted) = match expression {
        ParsedExpression::StringLiteral(_) => (Diagnostic::ts2872(ctx.file_name.clone()), true),
        ParsedExpression::NumberLiteral(value)
            if value.parse::<f64>().map_or(false, |n| n == 0.0) =>
        {
            (Diagnostic::ts2873(ctx.file_name.clone()), false)
        }
        ParsedExpression::NumberLiteral(_) => (Diagnostic::ts2872(ctx.file_name.clone()), true),
        ParsedExpression::BooleanLiteral(true) => (Diagnostic::ts2872(ctx.file_name.clone()), true),
        ParsedExpression::BooleanLiteral(false) => {
            (Diagnostic::ts2873(ctx.file_name.clone()), false)
        }
        ParsedExpression::UndefinedLiteral | ParsedExpression::NullLiteral => return false,
        _ => return false,
    };

    let diagnostic = match fallback_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };

    ctx.push(diagnostic);
    diagnostic_emitted
}

pub(crate) fn check_expression_flow_impl(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
    record_flow_expression_visit_count();
    if !flow_state.enabled || flow_state.tracked_local_count == 0 {
        return FlowCheck::Clear;
    }

    match expression {
        ParsedExpression::Identifier { name, span } => report_read_flow(
            name,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::Call {
            callee_name,
            callee_span,
            arguments,
            ..
        } => {
            if report_read_flow(
                callee_name,
                callee_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            for argument in arguments {
                if with_use_before_declaration_as_unassigned(true, || {
                    check_expression_flow_impl(
                        &argument.expression,
                        argument.span.or(fallback_span),
                        flow_state,
                        statement_index,
                        ctx,
                    )
                })
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::New {
            callee,
            callee_span,
            arguments,
            ..
        } => {
            if check_expression_flow_impl(
                callee,
                callee_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            for argument in arguments {
                if with_use_before_declaration_as_unassigned(true, || {
                    check_expression_flow_impl(
                        &argument.expression,
                        argument.span.or(fallback_span),
                        flow_state,
                        statement_index,
                        ctx,
                    )
                })
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::PropertyCall {
            object,
            object_span: _,
            arguments,
            ..
        } => {
            if check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx)
                .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            for argument in arguments {
                if with_use_before_declaration_as_unassigned(true, || {
                    check_expression_flow_impl(
                        &argument.expression,
                        argument.span.or(fallback_span),
                        flow_state,
                        statement_index,
                        ctx,
                    )
                })
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::PropertyAccess {
            object,
            object_span: _,
            ..
        } => check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx),
        ParsedExpression::Unary {
            operand,
            operand_span,
            ..
        } => check_expression_flow_impl(
            operand,
            operand_span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::Binary {
            left,
            left_span,
            right,
            right_span,
            ..
        } => {
            if check_expression_flow_impl(
                left,
                left_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                right,
                right_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::Logical {
            left,
            left_span,
            right,
            right_span,
            ..
        } => {
            if check_expression_flow_impl(
                left,
                left_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                right,
                right_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::Conditional {
            condition,
            condition_span,
            when_true,
            when_true_span,
            when_false,
            when_false_span,
        } => {
            if check_expression_flow_impl(
                condition,
                condition_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            if check_expression_flow_impl(
                when_true,
                when_true_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                when_false,
                when_false_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            for property in properties {
                if check_expression_flow_impl(
                    &property.value,
                    property.value_span.or(property.span).or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::ArrayLiteral { elements, .. } => {
            for element in elements {
                if check_expression_flow_impl(
                    &element.expression,
                    element.span.or(fallback_span),
                    flow_state,
                    statement_index,
                    ctx,
                )
                .is_blocked()
                {
                    return FlowCheck::Blocked;
                }
            }

            FlowCheck::Clear
        }
        ParsedExpression::IndexAccess {
            object_name,
            object_span,
            index,
            index_span,
        } => {
            if report_read_flow(
                object_name,
                object_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
            .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            check_expression_flow_impl(
                index,
                index_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::TypeAssertion {
            expression,
            expression_span,
            ..
        } => check_expression_flow_impl(
            expression,
            expression_span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::SatisfiesExpression {
            expression, span, ..
        } => check_expression_flow_impl(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::NonNullAssertion {
            expression, span, ..
        } => check_expression_flow_impl(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::ConstAssertion {
            expression, span, ..
        } => check_expression_flow_impl(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::OptionalPropertyAccess { object, .. } => {
            check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalIndexAccess { object, index, .. } => {
            let object_flow =
                check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx);
            if object_flow.is_blocked() {
                return object_flow;
            }
            check_expression_flow_impl(index, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalPropertyCall { object, .. } => {
            check_expression_flow_impl(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalCall { callee, .. } => {
            check_expression_flow_impl(callee, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::NullishCoalescing { left, right, .. } => {
            if check_expression_flow_impl(left, fallback_span, flow_state, statement_index, ctx)
                .is_blocked()
            {
                return FlowCheck::Blocked;
            }
            check_expression_flow_impl(right, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::ArrowFunction(_) => FlowCheck::Clear,
        ParsedExpression::StringLiteral(_)
        | ParsedExpression::NumberLiteral(_)
        | ParsedExpression::BooleanLiteral(_)
        | ParsedExpression::UndefinedLiteral
        | ParsedExpression::NullLiteral
        | ParsedExpression::Unknown => FlowCheck::Clear,
    }
}

pub(crate) fn check_assignment_target_flow(
    target_name: &str,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
    span: Option<SyntaxTextSpan>,
) -> FlowCheck {
    record_flow_identifier_read_count();
    if !flow_state.enabled || flow_state.tracked_local_count == 0 {
        return FlowCheck::Clear;
    }

    let FlowReadOutcome::UseBeforeDeclaration =
        flow_state.read_identifier(target_name, statement_index)
    else {
        return FlowCheck::Clear;
    };

    let mut diagnostic = Diagnostic::ts2448(target_name, ctx.file_name.clone());
    if let Some(span) = span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
    FlowCheck::Blocked
}

pub(crate) fn apply_variable_declaration_state(
    variable_kind: typescript_rust_syntax::ParsedVariableKind,
    variable_name: impl Into<Arc<str>>,
    has_initializer: bool,
    flow_state: &mut FunctionFlowState,
) {
    if !matches!(
        variable_kind,
        ParsedVariableKind::Let | ParsedVariableKind::Const
    ) {
        return;
    }

    let state = if has_initializer {
        AssignmentState::Assigned
    } else {
        AssignmentState::DeclaredUnassigned
    };

    flow_state.declare_current(variable_name, state);
}

pub(crate) fn mark_assignment_state(target_name: &str, flow_state: &mut FunctionFlowState) {
    flow_state.mark_assigned(target_name);
}

pub(crate) fn merge_branch_deltas(
    flow_state: &mut FunctionFlowState,
    branches: &[FlowBranchDelta],
    has_fallthrough_base: bool,
) {
    record_flow_branch_merge_count(flow_state.scopes.len());

    if !flow_state.enabled || flow_state.tracked_local_count == 0 {
        return;
    }

    let continuing_branches: Vec<&FlowBranchDelta> =
        branches.iter().filter(|branch| branch.continues).collect();

    if continuing_branches.is_empty() || continuing_branches.iter().all(|branch| branch.is_empty())
    {
        record_flow_branch_merge_fast_path_count();
        return;
    }

    let mut updates: Vec<(usize, Vec<(Arc<str>, AssignmentState)>)> = Vec::new();
    let mut total_updates = 0usize;

    for scope_index in 0..flow_state.scopes.len() {
        let Some(base_scope) = flow_state.scopes.get(scope_index) else {
            continue;
        };

        let mut scope_updates = Vec::new();

        for (name, base_state) in &base_scope.locals {
            record_flow_branch_merge_local_iteration_count(1);

            let mut assigned_branches = 0usize;
            for branch in &continuing_branches {
                let branch_state = branch.get(scope_index, name).unwrap_or(*base_state);
                if matches!(branch_state, AssignmentState::Assigned) {
                    assigned_branches += 1;
                }
            }

            let merged_state = match base_state {
                AssignmentState::Assigned => AssignmentState::Assigned,
                AssignmentState::DeclaredUnassigned => {
                    if has_fallthrough_base {
                        AssignmentState::DeclaredUnassigned
                    } else if continuing_branches.len() >= 1
                        && assigned_branches == continuing_branches.len()
                    {
                        AssignmentState::Assigned
                    } else {
                        AssignmentState::DeclaredUnassigned
                    }
                }
            };

            if merged_state != *base_state {
                total_updates += 1;
                scope_updates.push((name.clone(), merged_state));
            }
        }

        if !scope_updates.is_empty() {
            updates.push((scope_index, scope_updates));
        }
    }

    if total_updates == 0 {
        record_flow_branch_merge_fast_path_count();
        return;
    }

    for (scope_index, scope_updates) in updates {
        if let Some(scope) = flow_state.scopes.get_mut(scope_index) {
            for (name, merged_state) in scope_updates {
                scope.locals.insert(name, merged_state);
            }
        }
    }
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

fn report_read_flow(
    name: &str,
    span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
    record_flow_identifier_read_count();
    if !flow_state.enabled || flow_state.tracked_local_count == 0 {
        return FlowCheck::Clear;
    }

    match flow_state.read_identifier(name, statement_index) {
        FlowReadOutcome::Unresolved | FlowReadOutcome::Declared(AssignmentState::Assigned) => {
            FlowCheck::Clear
        }
        FlowReadOutcome::Declared(AssignmentState::DeclaredUnassigned) => {
            let mut diagnostic = Diagnostic::ts2454(name, ctx.file_name.clone());
            if let Some(span) = span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }

            ctx.push(diagnostic);
            FlowCheck::Blocked
        }
        FlowReadOutcome::UseBeforeDeclaration => {
            let mut diagnostic = Diagnostic::ts2448(name, ctx.file_name.clone());
            if let Some(span) = span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }

            ctx.push(diagnostic);
            if should_emit_use_before_declaration_as_unassigned() {
                let mut diagnostic = Diagnostic::ts2454(name, ctx.file_name.clone());
                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }

                ctx.push(diagnostic);
            }
            FlowCheck::Blocked
        }
    }
}

fn collect_function_flow_facts_from_body(
    body: &[ParsedFunctionBodyStatement],
    facts: &mut FunctionFlowFacts,
) {
    for statement in body {
        collect_function_flow_facts_from_statement(statement, facts);
    }
}

fn collect_function_flow_facts_from_statement(
    statement: &ParsedFunctionBodyStatement,
    facts: &mut FunctionFlowFacts,
) {
    facts.has_branching |= matches!(
        statement,
        ParsedFunctionBodyStatement::If(_)
            | ParsedFunctionBodyStatement::While(_)
            | ParsedFunctionBodyStatement::ForOf(_)
            | ParsedFunctionBodyStatement::Switch(_)
            | ParsedFunctionBodyStatement::Try(_)
    );

    match statement {
        ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
            if matches!(
                variable.kind,
                ParsedVariableKind::Let | ParsedVariableKind::Const
            ) {
                facts.has_let_or_const = true;
                facts.has_future_block_scoped_declarations = true;
                facts.has_uninitialized_let_or_const |= variable.initializer.is_none();
            }
            facts.has_identifier_reads |= variable.initializer.is_some();
            facts.has_assignments |= variable.initializer.is_some();
        }
        ParsedFunctionBodyStatement::Return(return_statement) => {
            facts.has_return_or_throw = true;
            facts.has_identifier_reads |= return_statement.expression.is_some();
        }
        ParsedFunctionBodyStatement::Throw(_) => {
            facts.has_return_or_throw = true;
            facts.has_identifier_reads = true;
        }
        ParsedFunctionBodyStatement::Assignment(_) => {
            facts.has_assignments = true;
            facts.has_identifier_reads = true;
        }
        ParsedFunctionBodyStatement::Expression(_) => {
            facts.has_identifier_reads = true;
        }
        ParsedFunctionBodyStatement::Block(block_body) => {
            collect_function_flow_facts_from_body(block_body, facts);
        }
        ParsedFunctionBodyStatement::If(if_statement) => {
            facts.has_identifier_reads = true;
            collect_function_flow_facts_from_body(&if_statement.then_body, facts);
            collect_function_flow_facts_from_body(&if_statement.else_body, facts);
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            facts.has_identifier_reads = true;
            collect_function_flow_facts_from_body(&while_statement.body, facts);
        }
        ParsedFunctionBodyStatement::ForOf(for_of_statement) => {
            facts.has_identifier_reads = true;
            collect_function_flow_facts_from_body(&for_of_statement.body, facts);
        }
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            facts.has_identifier_reads = true;
            for case in &switch_statement.cases {
                collect_function_flow_facts_from_body(&case.consequent, facts);
            }
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            facts.has_identifier_reads = true;
            collect_function_flow_facts_from_body(&try_statement.block, facts);
            if let Some(handler) = &try_statement.handler {
                collect_function_flow_facts_from_body(&handler.body, facts);
            }
            collect_function_flow_facts_from_body(&try_statement.finalizer, facts);
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ReturnFlowSummary {
    contains_value_return: bool,
    contains_throw: bool,
    guarantees_value_return: bool,
}

fn summarize_function_body_flow(body: &[ParsedFunctionBodyStatement]) -> ReturnFlowSummary {
    record_flow_return_analysis_walk_count();

    let mut summary = ReturnFlowSummary::default();
    for statement in body {
        let statement_summary = summarize_function_statement_flow(statement);
        summary.contains_value_return |= statement_summary.contains_value_return;
        summary.contains_throw |= statement_summary.contains_throw;
        summary.guarantees_value_return |= statement_summary.guarantees_value_return;
    }

    summary
}

fn summarize_function_statement_flow(statement: &ParsedFunctionBodyStatement) -> ReturnFlowSummary {
    match statement {
        ParsedFunctionBodyStatement::Return(return_statement) => ReturnFlowSummary {
            contains_value_return: return_statement.expression.is_some(),
            contains_throw: false,
            guarantees_value_return: return_statement.expression.is_some(),
        },
        ParsedFunctionBodyStatement::Throw(_) => ReturnFlowSummary {
            contains_value_return: true,
            contains_throw: true,
            guarantees_value_return: true,
        },
        ParsedFunctionBodyStatement::Block(block_body) => summarize_function_body_flow(block_body),
        ParsedFunctionBodyStatement::If(if_statement) => {
            let then_summary = summarize_function_body_flow(&if_statement.then_body);
            let else_summary = summarize_function_body_flow(&if_statement.else_body);

            ReturnFlowSummary {
                contains_value_return: then_summary.contains_value_return
                    || else_summary.contains_value_return,
                contains_throw: then_summary.contains_throw || else_summary.contains_throw,
                guarantees_value_return: !if_statement.else_body.is_empty()
                    && then_summary.guarantees_value_return
                    && else_summary.guarantees_value_return,
            }
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            let body_summary = summarize_function_body_flow(&while_statement.body);
            ReturnFlowSummary {
                contains_value_return: body_summary.contains_value_return,
                contains_throw: body_summary.contains_throw,
                guarantees_value_return: false,
            }
        }
        ParsedFunctionBodyStatement::ForOf(for_of_statement) => {
            let body_summary = summarize_function_body_flow(&for_of_statement.body);
            ReturnFlowSummary {
                contains_value_return: body_summary.contains_value_return,
                contains_throw: body_summary.contains_throw,
                guarantees_value_return: false,
            }
        }
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            let mut contains_value_return = false;
            let mut contains_throw = false;
            let mut guarantees_value_return = !switch_statement.cases.is_empty();

            for case in &switch_statement.cases {
                let case_summary = summarize_function_body_flow(&case.consequent);
                contains_value_return |= case_summary.contains_value_return;
                contains_throw |= case_summary.contains_throw;
                guarantees_value_return &= case_summary.guarantees_value_return;
            }

            ReturnFlowSummary {
                contains_value_return,
                contains_throw,
                guarantees_value_return,
            }
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            let block_summary = summarize_function_body_flow(&try_statement.block);
            let handler_summary = try_statement
                .handler
                .as_ref()
                .map(|handler| summarize_function_body_flow(&handler.body));
            let finalizer_summary = summarize_function_body_flow(&try_statement.finalizer);

            let handler_guarantees = handler_summary
                .as_ref()
                .is_none_or(|summary| summary.guarantees_value_return);

            ReturnFlowSummary {
                contains_value_return: block_summary.contains_value_return
                    || handler_summary
                        .as_ref()
                        .is_some_and(|summary| summary.contains_value_return)
                    || finalizer_summary.contains_value_return,
                contains_throw: block_summary.contains_throw
                    || handler_summary
                        .as_ref()
                        .is_some_and(|summary| summary.contains_throw)
                    || finalizer_summary.contains_throw,
                guarantees_value_return: handler_guarantees
                    && (block_summary.guarantees_value_return || block_summary.contains_throw),
            }
        }
        ParsedFunctionBodyStatement::VariableDeclaration(_)
        | ParsedFunctionBodyStatement::Assignment(_)
        | ParsedFunctionBodyStatement::Expression(_) => ReturnFlowSummary::default(),
    }
}
