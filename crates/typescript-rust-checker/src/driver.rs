use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode};
use typescript_rust_syntax::{
    ParsedInterfaceDeclaration, ParsedStatement, ParsedTypeAliasDeclaration, parse_source,
};

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::CheckerContext;
use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

pub fn check_source(source_text: &str, file_name: &str) -> Vec<Diagnostic> {
    check_source_with_options(
        source_text,
        file_name,
        crate::context::CheckerOptions::default(),
    )
}

pub fn check_source_with_options(
    source_text: &str,
    file_name: &str,
    options: crate::context::CheckerOptions,
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

    collect_type_declarations(&parsed.statements, &mut ctx);

    for statement in parsed.statements {
        match statement {
            ParsedStatement::VariableDeclaration(variable) => {
                var::check_variable_declaration(variable, &mut ctx);
            }
            ParsedStatement::Assignment(assignment) => {
                assign::check_assignment(assignment, &mut ctx);
            }
            ParsedStatement::FunctionDeclaration(function) => {
                check_function::check_function_declaration(function, &mut ctx);
            }
            ParsedStatement::Call(call) => {
                call::check_call(call, &mut ctx);
            }
            ParsedStatement::Expression(expression) => {
                expr::check_expression_statement(expression, &mut ctx);
            }
            ParsedStatement::TypeAliasDeclaration(_) => {}
            ParsedStatement::InterfaceDeclaration(_) => {}
        }
    }

    ctx.finish()
}

pub(crate) fn collect_type_declarations(statements: &[ParsedStatement], ctx: &mut CheckerContext) {
    for statement in statements {
        match statement {
            ParsedStatement::TypeAliasDeclaration(alias) => {
                collect_type_alias(alias, ctx);
            }
            ParsedStatement::InterfaceDeclaration(interface) => {
                collect_interface(interface, ctx);
            }
            _ => {}
        };
    }
}

pub(crate) fn collect_type_alias(alias: &ParsedTypeAliasDeclaration, ctx: &mut CheckerContext) {
    let info = TypeAliasInfo {
        name: alias.name.clone(),
        name_span: alias.name_span,
        ty: alias.ty.clone(),
    };

    if ctx
        .type_declarations
        .insert(alias.name.clone(), TypeDeclarationInfo::Alias(info))
        .is_some()
    {
        let mut diagnostic = Diagnostic::ts2300(&alias.name, ctx.file_name.clone());

        if let Some(span) = alias.name_span {
            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
        }

        ctx.push(diagnostic);
    }
}

pub(crate) fn collect_interface(interface: &ParsedInterfaceDeclaration, ctx: &mut CheckerContext) {
    let info = InterfaceInfo {
        name: interface.name.clone(),
        name_span: interface.name_span,
        members: interface.members.clone(),
    };

    if ctx
        .type_declarations
        .insert(interface.name.clone(), TypeDeclarationInfo::Interface(info))
        .is_some()
    {
        let mut diagnostic = Diagnostic::ts2300(&interface.name, ctx.file_name.clone());

        if let Some(span) = interface.name_span {
            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
        }

        ctx.push(diagnostic);
    }
}
