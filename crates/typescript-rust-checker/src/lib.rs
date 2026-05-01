mod check_assign;
mod check_call;
mod check_expr;
mod check_expr_ops;
mod check_function;
mod check_var;
mod context;
mod infer;
mod symbols;

use context::CheckerContext;
pub use context::CheckerOptions;
use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode};
use typescript_rust_syntax::{ParsedStatement, parse_source};

pub fn check_source(source_text: &str, file_name: &str) -> Vec<Diagnostic> {
    check_source_with_options(source_text, file_name, CheckerOptions::default())
}

pub fn check_source_with_options(
    source_text: &str,
    file_name: &str,
    options: CheckerOptions,
) -> Vec<Diagnostic> {
    let parsed = parse_source(source_text, file_name);
    let file_name = parsed.file_name;
    let mut ctx = CheckerContext::new(file_name.clone(), options);

    for message in parsed.parser_errors {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::Custom("typescript-rust::parser-error"),
            message,
            file_name.clone(),
        );
        ctx.push(diagnostic);
    }

    for statement in parsed.statements {
        match statement {
            ParsedStatement::VariableDeclaration(variable) => {
                check_var::check_variable_declaration(variable, &mut ctx);
            }
            ParsedStatement::Assignment(assignment) => {
                check_assign::check_assignment(assignment, &mut ctx);
            }
            ParsedStatement::FunctionDeclaration(function) => {
                check_function::check_function_declaration(function, &mut ctx);
            }
            ParsedStatement::Call(call) => {
                check_call::check_call(call, &mut ctx);
            }
            ParsedStatement::Expression(expression) => {
                check_expr::check_expression_statement(expression, &mut ctx);
            }
        }
    }

    ctx.finish()
}
