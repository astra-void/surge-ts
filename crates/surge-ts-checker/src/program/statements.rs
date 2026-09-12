//! Per-statement program checking and unsupported-declaration diagnostics.

use std::collections::HashMap;
use std::time::Instant;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedFunctionDeclaration,
    ParsedImportKind, ParsedStatement, TextSpan,
};
use surge_ts_types::FunctionType;

use super::*;

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::CheckerContext;

pub(crate) fn check_program_file_statements(
    statements: &[ParsedStatement],
    file_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (statement_index, statement) in statements.iter().cloned().enumerate() {
        check_program_statement(
            statement,
            file_index,
            statement_index,
            function_signatures,
            ctx,
        );
    }
}

/// A module-scope `if`: the condition is checked, and when one branch cannot
/// fall through (`if (isCancel(value)) process.exit(0)`) the statements after
/// it see the other branch's narrowing, exactly as a function body would.
/// The branch bodies themselves are not checked here — module scope has no
/// flow state to check them under yet — which is what happened to them before
/// the parser kept them at all.
pub(crate) fn check_module_if_statement(
    if_statement: &surge_ts_syntax::ParsedIfStatement,
    ctx: &mut CheckerContext,
) {
    let symbols = ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    // The condition is evaluated for its narrowing only. Reporting it would
    // change what a module-scope `if` costs relative to the statements around
    // it: its branches are not checked yet, and a condition that reads a value
    // surge models more loosely than tsc (`args.verbose` off a parsed-options
    // object that degraded to an index signature) would report where tsc
    // does not. Its diagnostics are discarded like a probe's.
    let checkpoint = ctx.diagnostics().len();
    let _ = crate::checks::expr::evaluate_expression(
        &if_statement.condition,
        if_statement.condition_span,
        &symbols,
        ctx,
    );
    ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
    let scopes = crate::symbols::ScopeStack::from_root(symbols);
    let diverts = |body: &[surge_ts_syntax::ParsedFunctionBodyStatement]| {
        let flow = crate::flow::analyze_function_body_flow(body);
        flow.guarantees_value_return
            || flow.guarantees_exit
            || crate::checks::function::body_ends_in_never_call(body, &scopes)
    };
    let then_diverts = diverts(&if_statement.then_body);
    let else_diverts = !if_statement.else_body.is_empty() && diverts(&if_statement.else_body);
    let surviving_branch = match (then_diverts, else_diverts) {
        (true, false) => false,
        (false, true) => true,
        _ => return,
    };
    let base = ctx.symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    let narrowed = crate::checks::function::narrow_condition_symbol_table(
        &if_statement.condition,
        &base,
        surviving_branch,
    );
    let narrowed = crate::checks::function::narrow_predicate_guards_symbol_table(
        &if_statement.condition,
        narrowed.as_ref().unwrap_or(&base),
        surviving_branch,
        ctx,
    )
    .or(narrowed);
    if let Some(narrowed) = narrowed {
        ctx.symbols = narrowed;
    }
}

pub(crate) fn check_program_statement(
    statement: ParsedStatement,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            let start = Instant::now();
            var::check_variable_declaration(*variable, ctx);
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.variable_declaration_checking += start.elapsed()
            });
        }
        ParsedStatement::Assignment(assignment) => {
            assign::check_assignment(*assignment, ctx);
        }
        ParsedStatement::FunctionDeclaration(function) => {
            check_program_function_declaration(
                *function,
                file_index,
                statement_index,
                function_signatures,
                ctx,
            );
        }
        ParsedStatement::Call(call) => {
            call::check_call(*call, ctx);
        }
        ParsedStatement::Expression(expression) => {
            expr::check_expression_statement(*expression, ctx);
        }
        ParsedStatement::If(if_statement) => check_module_if_statement(&if_statement, ctx),
        ParsedStatement::TypeAliasDeclaration(_) => {}
        ParsedStatement::InterfaceDeclaration(_) => {}
        ParsedStatement::ClassDeclaration(class) => {
            super::check_class_declaration(&class, ctx);
        }
        ParsedStatement::ImportDeclaration(_) => {}
        ParsedStatement::ExportDeclaration(export) => match *export {
            ParsedExportDeclaration::Statement { declaration, .. } => check_program_statement(
                *declaration,
                file_index,
                statement_index,
                function_signatures,
                ctx,
            ),
            ParsedExportDeclaration::Named { .. } => {}
            ParsedExportDeclaration::Namespace { .. } => {}
            ParsedExportDeclaration::Default { declaration, .. } => match declaration {
                ParsedDefaultExportDeclaration::Function(function) => {
                    check_program_function_declaration(
                        function,
                        file_index,
                        statement_index,
                        function_signatures,
                        ctx,
                    );
                }
                ParsedDefaultExportDeclaration::Expression(expression) => {
                    expr::check_expression_statement(expression, ctx);
                }
                ParsedDefaultExportDeclaration::Class(class) => {
                    super::check_class_declaration(&class, ctx);
                }
                ParsedDefaultExportDeclaration::Unsupported { span } => {
                    let mut diagnostic =
                        Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                    if let Some(span) = span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }

                    ctx.push(diagnostic);
                }
            },
            ParsedExportDeclaration::All { .. } => {}
            ParsedExportDeclaration::Empty { .. } => {}
            ParsedExportDeclaration::Equals { .. } => {}
            ParsedExportDeclaration::NamespaceExport { .. } => {}
            ParsedExportDeclaration::Unsupported { span } => {
                let mut diagnostic =
                    Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        },
        ParsedStatement::DeclareModuleDeclaration(_) => {}
        // Namespace members are bound during type-declaration collection; the
        // namespace itself produces no value-level checks here.
        ParsedStatement::NamespaceDeclaration(_) => {}
        ParsedStatement::UnsupportedDeclaration { span } => {
            emit_unsupported_declaration_diagnostic(ctx, span);
        }
    }
}

pub(crate) fn emit_unsupported_declaration_diagnostics(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        emit_unsupported_declaration_diagnostic_from_statement(statement, ctx);
    }
}

pub(crate) fn emit_unsupported_declaration_diagnostic_from_statement(
    statement: &ParsedStatement,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::UnsupportedDeclaration { span } => {
            emit_unsupported_declaration_diagnostic(ctx, *span);
        }
        ParsedStatement::ImportDeclaration(import)
            if matches!(
                import.kind,
                ParsedImportKind::Unsupported | ParsedImportKind::TypeOnlyDefault { .. }
            ) =>
        {
            emit_unsupported_declaration_diagnostic(
                ctx,
                import.span.or(import.module_specifier_span),
            );
        }
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Unsupported { span } => {
                emit_unsupported_declaration_diagnostic(ctx, *span);
            }
            ParsedExportDeclaration::Default {
                declaration: ParsedDefaultExportDeclaration::Unsupported { span },
                span: declaration_span,
            } => {
                emit_unsupported_declaration_diagnostic(ctx, (*span).or(*declaration_span));
            }
            _ => {}
        },
        ParsedStatement::DeclareModuleDeclaration(module) => {
            emit_unsupported_declaration_diagnostics(&module.statements, ctx);
        }
        _ => {}
    }
}

pub(crate) fn emit_unsupported_declaration_diagnostic(
    ctx: &mut CheckerContext,
    span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::surge_unsupported_declaration(ctx.file_name.clone());

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn check_program_function_declaration(
    function: ParsedFunctionDeclaration,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    let declaration_location = FunctionDeclarationLocation {
        file_index,
        statement_index,
    };

    let saved_symbols = std::mem::take(&mut ctx.symbols);
    let body_root_symbols =
        saved_symbols.clone_with_reason(surge_ts_types::TypeCopyReason::FunctionBodySetup);
    ctx.symbols = body_root_symbols;
    let Some(function_type) = function_signatures.get(&declaration_location) else {
        check_function::check_function_declaration(function, ctx);
        ctx.symbols = saved_symbols;
        return;
    };

    let type_parameters = function.type_parameters.clone();
    check_function::check_function_declaration_body(function, function_type, &type_parameters, ctx);
    ctx.symbols = saved_symbols;
}

pub(crate) fn count_local_type_declarations_in_statements(statements: &[ParsedStatement]) -> usize {
    statements
        .iter()
        .map(count_local_type_declarations_in_statement)
        .sum()
}

pub(crate) fn count_local_type_declarations_in_statement(statement: &ParsedStatement) -> usize {
    match statement {
        ParsedStatement::TypeAliasDeclaration(_) => 1,
        ParsedStatement::InterfaceDeclaration(_) => 1,
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                count_local_type_declarations_in_statement(declaration.as_ref())
            }
            _ => 0,
        },
        _ => 0,
    }
}
