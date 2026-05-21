use std::collections::BTreeMap;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedExpression, ParsedFunctionBodyStatement, ParsedVariableKind, TextSpan as SyntaxTextSpan,
};

use crate::context::{CheckerContext, convert_span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionBodyFlow {
    pub(crate) contains_value_return: bool,
    pub(crate) guarantees_value_return: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssignmentState {
    DeclaredUnassigned,
    MaybeAssigned,
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

#[derive(Debug, Clone)]
pub(crate) struct FunctionFlowState {
    scopes: Vec<FlowScope>,
}

#[derive(Debug, Clone, Default)]
struct FlowScope {
    locals: BTreeMap<String, AssignmentState>,
    future_block_scoped_declarations: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowReadOutcome {
    Unresolved,
    UseBeforeDeclaration,
    Declared(AssignmentState),
}

pub(crate) fn analyze_function_body_flow(body: &[ParsedFunctionBodyStatement]) -> FunctionBodyFlow {
    FunctionBodyFlow {
        contains_value_return: body.iter().any(function_statement_contains_value_return),
        guarantees_value_return: body.iter().any(function_statement_guarantees_value_return),
    }
}

pub(crate) fn collect_future_block_scoped_declarations(
    body: &[ParsedFunctionBodyStatement],
) -> BTreeMap<String, usize> {
    let mut declarations = BTreeMap::new();

    for (index, statement) in body.iter().enumerate() {
        if let ParsedFunctionBodyStatement::VariableDeclaration(variable) = statement {
            if matches!(
                variable.kind,
                ParsedVariableKind::Let | ParsedVariableKind::Const
            ) {
                declarations.entry(variable.name.clone()).or_insert(index);
            }
        }
    }

    declarations
}

pub(crate) fn check_obvious_truthiness_condition(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
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
        ParsedExpression::UndefinedLiteral => return false,
        _ => return false,
    };

    let diagnostic = match fallback_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };

    ctx.push(diagnostic);
    diagnostic_emitted
}

pub(crate) fn check_expression_flow(
    expression: &ParsedExpression,
    fallback_span: Option<SyntaxTextSpan>,
    flow_state: &FunctionFlowState,
    statement_index: usize,
    ctx: &mut CheckerContext,
) -> FlowCheck {
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
                if check_expression_flow(
                    &argument.expression,
                    argument.span.or(fallback_span),
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
        ParsedExpression::New {
            callee,
            callee_span,
            arguments,
            ..
        } => {
            if check_expression_flow(
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
                if check_expression_flow(
                    &argument.expression,
                    argument.span.or(fallback_span),
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
        ParsedExpression::PropertyCall {
            object,
            object_span: _,
            arguments,
            ..
        } => {
            if check_expression_flow(object, fallback_span, flow_state, statement_index, ctx)
                .is_blocked()
            {
                return FlowCheck::Blocked;
            }

            for argument in arguments {
                if check_expression_flow(
                    &argument.expression,
                    argument.span.or(fallback_span),
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
        ParsedExpression::PropertyAccess {
            object,
            object_span: _,
            ..
        } => check_expression_flow(object, fallback_span, flow_state, statement_index, ctx),
        ParsedExpression::Unary {
            operand,
            operand_span,
            ..
        } => check_expression_flow(
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
            if check_expression_flow(
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

            check_expression_flow(
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
            if check_expression_flow(
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

            check_expression_flow(
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
            if check_expression_flow(
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

            if check_expression_flow(
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

            check_expression_flow(
                when_false,
                when_false_span.or(fallback_span),
                flow_state,
                statement_index,
                ctx,
            )
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            for property in properties {
                if check_expression_flow(
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
                if check_expression_flow(
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

            check_expression_flow(
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
        } => check_expression_flow(
            expression,
            expression_span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::SatisfiesExpression {
            expression, span, ..
        } => check_expression_flow(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::NonNullAssertion {
            expression, span, ..
        } => check_expression_flow(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::ConstAssertion {
            expression, span, ..
        } => check_expression_flow(
            expression,
            span.or(fallback_span),
            flow_state,
            statement_index,
            ctx,
        ),
        ParsedExpression::OptionalPropertyAccess { object, .. } => {
            check_expression_flow(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalIndexAccess { object, index, .. } => {
            let object_flow =
                check_expression_flow(object, fallback_span, flow_state, statement_index, ctx);
            if object_flow.is_blocked() {
                return object_flow;
            }
            check_expression_flow(index, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalPropertyCall { object, .. } => {
            check_expression_flow(object, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::OptionalCall { callee, .. } => {
            check_expression_flow(callee, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::NullishCoalescing { left, right, .. } => {
            if check_expression_flow(left, fallback_span, flow_state, statement_index, ctx)
                .is_blocked()
            {
                return FlowCheck::Blocked;
            }
            check_expression_flow(right, fallback_span, flow_state, statement_index, ctx)
        }
        ParsedExpression::ArrowFunction(_) => FlowCheck::Clear,
        ParsedExpression::StringLiteral(_)
        | ParsedExpression::NumberLiteral(_)
        | ParsedExpression::BooleanLiteral(_)
        | ParsedExpression::UndefinedLiteral
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
    variable_name: String,
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

pub(crate) fn merge_branch_states(
    base: &FunctionFlowState,
    branches: &[FunctionFlowState],
) -> FunctionFlowState {
    let mut merged = base.clone();

    for scope_index in 0..base.scopes.len() {
        let base_scope = &base.scopes[scope_index];
        let mut merged_locals = base_scope.locals.clone();

        for (name, base_state) in &base_scope.locals {
            let mut assigned_branches = 0usize;
            let mut maybe_branches = 0usize;

            for branch in branches {
                let branch_state = branch
                    .scopes
                    .get(scope_index)
                    .and_then(|scope| scope.locals.get(name))
                    .copied()
                    .unwrap_or(*base_state);

                match branch_state {
                    AssignmentState::Assigned => assigned_branches += 1,
                    AssignmentState::MaybeAssigned => maybe_branches += 1,
                    AssignmentState::DeclaredUnassigned => {}
                }
            }

            let merged_state = match base_state {
                AssignmentState::Assigned => AssignmentState::Assigned,
                AssignmentState::MaybeAssigned => AssignmentState::MaybeAssigned,
                AssignmentState::DeclaredUnassigned => {
                    if branches.is_empty() {
                        AssignmentState::DeclaredUnassigned
                    } else if assigned_branches == branches.len() && branches.len() > 1 {
                        AssignmentState::Assigned
                    } else if assigned_branches > 0 || maybe_branches > 0 {
                        // Keep single-branch assignment conservative for now. This avoids
                        // escalating to TS2454 on paths where the variable may still be unset.
                        AssignmentState::MaybeAssigned
                    } else {
                        AssignmentState::DeclaredUnassigned
                    }
                }
            };

            merged_locals.insert(name.clone(), merged_state);
        }

        merged.scopes[scope_index].locals = merged_locals;
    }

    merged
}

impl FunctionFlowState {
    pub(crate) fn new() -> Self {
        Self { scopes: Vec::new() }
    }

    pub(crate) fn push_scope(&mut self, future_block_scoped_declarations: BTreeMap<String, usize>) {
        self.scopes.push(FlowScope {
            locals: BTreeMap::new(),
            future_block_scoped_declarations,
        });
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub(crate) fn declare_current(&mut self, name: String, state: AssignmentState) {
        let scope = self
            .scopes
            .last_mut()
            .expect("flow state must contain at least one scope");
        scope.locals.insert(name, state);
    }

    pub(crate) fn mark_assigned(&mut self, name: &str) {
        if let Some(scope) = self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.locals.contains_key(name))
        {
            scope
                .locals
                .insert(name.to_string(), AssignmentState::Assigned);
        }
    }

    fn read_identifier(&self, name: &str, statement_index: usize) -> FlowReadOutcome {
        let Some(current_scope) = self.scopes.last() else {
            return FlowReadOutcome::Unresolved;
        };

        if let Some(state) = current_scope.locals.get(name).copied() {
            return FlowReadOutcome::Declared(state);
        }

        if current_scope
            .future_block_scoped_declarations
            .get(name)
            .is_some_and(|declaration_index| statement_index < *declaration_index)
        {
            return FlowReadOutcome::UseBeforeDeclaration;
        }

        for scope in self.scopes.iter().rev().skip(1) {
            if let Some(state) = scope.locals.get(name).copied() {
                return FlowReadOutcome::Declared(state);
            }
        }

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
    match flow_state.read_identifier(name, statement_index) {
        FlowReadOutcome::Unresolved | FlowReadOutcome::Declared(AssignmentState::Assigned) => {
            FlowCheck::Clear
        }
        FlowReadOutcome::Declared(AssignmentState::MaybeAssigned) => FlowCheck::Clear,
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
            FlowCheck::Blocked
        }
    }
}

fn function_body_contains_value_return(body: &[ParsedFunctionBodyStatement]) -> bool {
    body.iter().any(function_statement_contains_value_return)
}

fn function_body_contains_throw(body: &[ParsedFunctionBodyStatement]) -> bool {
    body.iter().any(function_statement_contains_throw)
}

fn function_statement_contains_value_return(statement: &ParsedFunctionBodyStatement) -> bool {
    match statement {
        ParsedFunctionBodyStatement::Return(return_statement) => {
            return_statement.expression.is_some()
        }
        ParsedFunctionBodyStatement::Throw(_) => true,
        ParsedFunctionBodyStatement::Block(block_body) => {
            function_body_contains_value_return(block_body)
        }
        ParsedFunctionBodyStatement::If(if_statement) => {
            function_body_contains_value_return(&if_statement.then_body)
                || function_body_contains_value_return(&if_statement.else_body)
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            function_body_contains_value_return(&while_statement.body)
        }
        ParsedFunctionBodyStatement::Switch(switch_statement) => switch_statement
            .cases
            .iter()
            .any(|case| function_body_contains_value_return(&case.consequent)),
        ParsedFunctionBodyStatement::Try(try_statement) => {
            function_body_contains_value_return(&try_statement.block)
                || try_statement
                    .handler
                    .as_ref()
                    .is_some_and(|handler| function_body_contains_value_return(&handler.body))
                || function_body_contains_value_return(&try_statement.finalizer)
        }
        ParsedFunctionBodyStatement::VariableDeclaration(_)
        | ParsedFunctionBodyStatement::Assignment(_)
        | ParsedFunctionBodyStatement::Expression(_) => false,
    }
}

fn function_statement_contains_throw(statement: &ParsedFunctionBodyStatement) -> bool {
    match statement {
        ParsedFunctionBodyStatement::Throw(_) => true,
        ParsedFunctionBodyStatement::Block(block_body) => function_body_contains_throw(block_body),
        ParsedFunctionBodyStatement::If(if_statement) => {
            function_body_contains_throw(&if_statement.then_body)
                || function_body_contains_throw(&if_statement.else_body)
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            function_body_contains_throw(&while_statement.body)
        }
        ParsedFunctionBodyStatement::Switch(switch_statement) => switch_statement
            .cases
            .iter()
            .any(|case| function_body_contains_throw(&case.consequent)),
        ParsedFunctionBodyStatement::Try(try_statement) => {
            function_body_contains_throw(&try_statement.block)
                || try_statement
                    .handler
                    .as_ref()
                    .is_some_and(|handler| function_body_contains_throw(&handler.body))
                || function_body_contains_throw(&try_statement.finalizer)
        }
        ParsedFunctionBodyStatement::Return(_)
        | ParsedFunctionBodyStatement::VariableDeclaration(_)
        | ParsedFunctionBodyStatement::Assignment(_)
        | ParsedFunctionBodyStatement::Expression(_) => false,
    }
}

fn function_body_guarantees_value_return(body: &[ParsedFunctionBodyStatement]) -> bool {
    body.iter().any(function_statement_guarantees_value_return)
}

fn function_statement_guarantees_value_return(statement: &ParsedFunctionBodyStatement) -> bool {
    match statement {
        ParsedFunctionBodyStatement::Return(return_statement) => {
            return_statement.expression.is_some()
        }
        ParsedFunctionBodyStatement::Throw(_) => true,
        ParsedFunctionBodyStatement::Block(block_body) => {
            function_body_guarantees_value_return(block_body)
        }
        ParsedFunctionBodyStatement::If(if_statement) => {
            !if_statement.else_body.is_empty()
                && function_body_guarantees_value_return(&if_statement.then_body)
                && function_body_guarantees_value_return(&if_statement.else_body)
        }
        ParsedFunctionBodyStatement::While(_) => false,
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            !switch_statement.cases.is_empty()
                && switch_statement
                    .cases
                    .iter()
                    .all(|case| function_body_guarantees_value_return(&case.consequent))
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            let try_guarantees = function_body_guarantees_value_return(&try_statement.block);
            let handler_guarantees = try_statement
                .handler
                .as_ref()
                .is_none_or(|handler| function_body_guarantees_value_return(&handler.body));
            let try_throws = function_body_contains_throw(&try_statement.block);

            handler_guarantees && (try_guarantees || try_throws)
        }
        ParsedFunctionBodyStatement::VariableDeclaration(_)
        | ParsedFunctionBodyStatement::Assignment(_)
        | ParsedFunctionBodyStatement::Expression(_) => false,
    }
}
