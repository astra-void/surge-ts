//! Function flow-fact collection and return-flow summarization.

use super::*;

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedFunctionBodyStatement, ParsedVariableKind, TextSpan as SyntaxTextSpan,
};

use crate::context::{CheckerContext, convert_span};
use crate::program::{
    record_flow_future_declaration_collection_count, record_flow_identifier_read_count,
    record_flow_return_analysis_walk_count,
};

thread_local! {
    static EMIT_USE_BEFORE_DECLARATION_AS_UNASSIGNED: Cell<bool> = const { Cell::new(false) };
}

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

pub(crate) fn summarize_function_body_flow(
    body: &[ParsedFunctionBodyStatement],
) -> ReturnFlowSummary {
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

pub(crate) fn summarize_function_statement_flow(
    statement: &ParsedFunctionBodyStatement,
) -> ReturnFlowSummary {
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
