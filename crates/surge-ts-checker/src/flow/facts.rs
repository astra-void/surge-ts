//! Function flow-fact collection and return-flow summarization.

use super::*;

use std::collections::HashMap;
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedExpression, ParsedFunctionBodyStatement, ParsedVariableKind, TextSpan as SyntaxTextSpan,
};

use crate::context::{CheckerContext, convert_span};
use crate::program::{
    record_flow_future_declaration_collection_count, record_flow_identifier_read_count,
    record_flow_return_analysis_walk_count,
};

pub(crate) fn collect_function_flow_facts_from_body(
    body: &[ParsedFunctionBodyStatement],
    facts: &mut FunctionFlowFacts,
) {
    for statement in body {
        collect_function_flow_facts_from_statement(statement, facts);
    }
}

pub(crate) fn collect_function_flow_facts_from_statement(
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
        ParsedFunctionBodyStatement::Function(_) => {}
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
        ParsedFunctionBodyStatement::ThisPropertyAssignment(_) => {
            facts.has_assignments = true;
            facts.has_identifier_reads = true;
        }
        ParsedFunctionBodyStatement::Expression(_) => {
            facts.has_identifier_reads = true;
        }
        ParsedFunctionBodyStatement::Continue | ParsedFunctionBodyStatement::Break => {}
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

fn is_always_truthy_condition(condition: &ParsedExpression) -> bool {
    matches!(condition, ParsedExpression::BooleanLiteral(true))
}

/// Whether `body` contains a `break` that would exit the loop it directly
/// belongs to. Recurses into structured statements that share the loop's break
/// target (`if`/block/`try`) but not into nested loops or `switch`, which
/// capture their own `break`.
fn body_breaks_enclosing_loop(body: &[ParsedFunctionBodyStatement]) -> bool {
    body.iter().any(statement_breaks_enclosing_loop)
}

fn statement_breaks_enclosing_loop(statement: &ParsedFunctionBodyStatement) -> bool {
    match statement {
        ParsedFunctionBodyStatement::Break => true,
        ParsedFunctionBodyStatement::Block(block) => body_breaks_enclosing_loop(block),
        ParsedFunctionBodyStatement::If(if_statement) => {
            body_breaks_enclosing_loop(&if_statement.then_body)
                || body_breaks_enclosing_loop(&if_statement.else_body)
        }
        ParsedFunctionBodyStatement::Try(try_statement) => {
            body_breaks_enclosing_loop(&try_statement.block)
                || try_statement
                    .handler
                    .as_ref()
                    .is_some_and(|handler| body_breaks_enclosing_loop(&handler.body))
                || body_breaks_enclosing_loop(&try_statement.finalizer)
        }
        _ => false,
    }
}

pub(crate) fn summarize_function_body_flow(
    body: &[ParsedFunctionBodyStatement],
) -> ReturnFlowSummary {
    record_flow_return_analysis_walk_count();

    let mut summary = ReturnFlowSummary::default();
    for statement in body {
        let statement_summary = summarize_function_statement_flow(statement);
        summary.contains_value_return |= statement_summary.contains_value_return;
        summary.contains_return_with_value |= statement_summary.contains_return_with_value;
        summary.contains_throw |= statement_summary.contains_throw;
        summary.guarantees_value_return |= statement_summary.guarantees_value_return;
        summary.guarantees_exit |= statement_summary.guarantees_exit;
    }

    summary
}

pub(crate) fn summarize_function_statement_flow(
    statement: &ParsedFunctionBodyStatement,
) -> ReturnFlowSummary {
    match statement {
        ParsedFunctionBodyStatement::Function(_) => ReturnFlowSummary::default(),
        ParsedFunctionBodyStatement::Return(return_statement) => ReturnFlowSummary {
            contains_value_return: return_statement.expression.is_some(),
            contains_return_with_value: return_statement.expression.is_some(),
            contains_throw: false,
            guarantees_value_return: return_statement.expression.is_some(),
            guarantees_exit: true,
        },
        ParsedFunctionBodyStatement::Throw(_) => ReturnFlowSummary {
            contains_value_return: true,
            contains_return_with_value: false,
            contains_throw: true,
            guarantees_value_return: true,
            guarantees_exit: true,
        },
        ParsedFunctionBodyStatement::Continue | ParsedFunctionBodyStatement::Break => {
            ReturnFlowSummary {
                contains_value_return: false,
                contains_return_with_value: false,
                contains_throw: false,
                guarantees_value_return: false,
                guarantees_exit: true,
            }
        }
        ParsedFunctionBodyStatement::Block(block_body) => summarize_function_body_flow(block_body),
        ParsedFunctionBodyStatement::If(if_statement) => {
            let then_summary = summarize_function_body_flow(&if_statement.then_body);
            let else_summary = summarize_function_body_flow(&if_statement.else_body);

            ReturnFlowSummary {
                contains_value_return: then_summary.contains_value_return
                    || else_summary.contains_value_return,
                contains_return_with_value: then_summary.contains_return_with_value
                    || else_summary.contains_return_with_value,
                contains_throw: then_summary.contains_throw || else_summary.contains_throw,
                guarantees_value_return: !if_statement.else_body.is_empty()
                    && then_summary.guarantees_value_return
                    && else_summary.guarantees_value_return,
                guarantees_exit: !if_statement.else_body.is_empty()
                    && then_summary.guarantees_exit
                    && else_summary.guarantees_exit,
            }
        }
        ParsedFunctionBodyStatement::While(while_statement) => {
            let body_summary = summarize_function_body_flow(&while_statement.body);
            // An infinite loop (`while (true)`) with no `break` reaching it never
            // falls through, so control after the loop — including the function's
            // implicit end — is unreachable. Matching tsc's reachability lets the
            // missing-return analysis skip TS7030/TS2366 for these (and narrows
            // unreachable trailing code).
            let never_falls_through = is_always_truthy_condition(&while_statement.condition)
                && !body_breaks_enclosing_loop(&while_statement.body);
            ReturnFlowSummary {
                contains_value_return: body_summary.contains_value_return,
                contains_return_with_value: body_summary.contains_return_with_value,
                contains_throw: body_summary.contains_throw,
                guarantees_value_return: never_falls_through,
                guarantees_exit: never_falls_through,
            }
        }
        ParsedFunctionBodyStatement::ForOf(for_of_statement) => {
            let body_summary = summarize_function_body_flow(&for_of_statement.body);
            ReturnFlowSummary {
                contains_value_return: body_summary.contains_value_return,
                contains_return_with_value: body_summary.contains_return_with_value,
                contains_throw: body_summary.contains_throw,
                guarantees_value_return: false,
                guarantees_exit: false,
            }
        }
        ParsedFunctionBodyStatement::Switch(switch_statement) => {
            let mut contains_value_return = false;
            let mut contains_return_with_value = false;
            let mut contains_throw = false;
            let mut guarantees_value_return = !switch_statement.cases.is_empty();

            for case in &switch_statement.cases {
                let case_summary = summarize_function_body_flow(&case.consequent);
                contains_value_return |= case_summary.contains_value_return;
                contains_return_with_value |= case_summary.contains_return_with_value;
                contains_throw |= case_summary.contains_throw;
                guarantees_value_return &= case_summary.guarantees_value_return;
            }

            // A switch falls through to the following statement unless it is
            // exhaustive (has a `default`), no consequent `break`s out of it, and
            // every clause leaves via `return`/`throw` (empty clauses fall through
            // to the next, so only the last clause must itself terminate). Without
            // this the construct never reports a guaranteed exit, so an exhaustive
            // `switch` whose clauses all `return` looks like it falls through.
            let has_default = switch_statement
                .cases
                .iter()
                .any(|case| case.test.is_none());
            let breaks_out = switch_statement
                .cases
                .iter()
                .any(|case| body_breaks_enclosing_loop(&case.consequent));
            let clauses_terminate = switch_statement.cases.iter().all(|case| {
                case.consequent.is_empty()
                    || summarize_function_body_flow(&case.consequent).guarantees_exit
            });
            let last_clause_terminates = switch_statement.cases.last().is_some_and(|case| {
                !case.consequent.is_empty()
                    && summarize_function_body_flow(&case.consequent).guarantees_exit
            });
            let guarantees_exit =
                has_default && !breaks_out && clauses_terminate && last_clause_terminates;

            ReturnFlowSummary {
                contains_value_return,
                contains_return_with_value,
                contains_throw,
                guarantees_value_return,
                guarantees_exit,
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

            // The try completes normally only if its block completes normally
            // (and, when present, its handler does too on the throwing path). A
            // `return`/`throw` in the finalizer overrides everything. Without this
            // the construct never reports a guaranteed exit, so `try { return }
            // catch { throw }` looks like it falls through.
            let body_and_handler_exit = block_summary.guarantees_exit
                && handler_summary
                    .as_ref()
                    .is_none_or(|summary| summary.guarantees_exit);

            ReturnFlowSummary {
                contains_value_return: block_summary.contains_value_return
                    || handler_summary
                        .as_ref()
                        .is_some_and(|summary| summary.contains_value_return)
                    || finalizer_summary.contains_value_return,
                contains_return_with_value: block_summary.contains_return_with_value
                    || handler_summary
                        .as_ref()
                        .is_some_and(|summary| summary.contains_return_with_value)
                    || finalizer_summary.contains_return_with_value,
                contains_throw: block_summary.contains_throw
                    || handler_summary
                        .as_ref()
                        .is_some_and(|summary| summary.contains_throw)
                    || finalizer_summary.contains_throw,
                guarantees_value_return: handler_guarantees
                    && (block_summary.guarantees_value_return || block_summary.contains_throw),
                guarantees_exit: body_and_handler_exit || finalizer_summary.guarantees_exit,
            }
        }
        ParsedFunctionBodyStatement::VariableDeclaration(_)
        | ParsedFunctionBodyStatement::Assignment(_)
        | ParsedFunctionBodyStatement::ThisPropertyAssignment(_)
        | ParsedFunctionBodyStatement::Expression(_) => ReturnFlowSummary::default(),
    }
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

pub(crate) fn report_read_flow(
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
            // A block-scoped (`let`/`const`) variable read before its declaration
            // is necessarily in its temporal dead zone, so it is also definitely
            // unassigned. tsc reports both TS2448 and TS2454 at every such read.
            let mut diagnostic = Diagnostic::ts2448(name, ctx.file_name.clone());
            if let Some(span) = span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push(diagnostic);

            let mut diagnostic = Diagnostic::ts2454(name, ctx.file_name.clone());
            if let Some(span) = span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push(diagnostic);

            FlowCheck::Blocked
        }
    }
}
