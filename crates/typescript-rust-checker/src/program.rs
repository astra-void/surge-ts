use std::collections::HashMap;

use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode};
use typescript_rust_syntax::{
    ParsedExportDeclaration, ParsedFunctionDeclaration, ParsedStatement, parse_source,
};
use typescript_rust_types::FunctionType;

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::{CheckerContext, CheckerOptions};
use crate::driver::collect_type_declarations;
use crate::symbols::{SymbolTable, TypeDeclarationTable};

#[derive(Debug, Clone)]
pub struct SourceFileInput {
    pub file_name: String,
    pub source_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FunctionDeclarationLocation {
    file_index: usize,
    statement_index: usize,
}

#[derive(Debug, Clone)]
struct ParsedProgramFile {
    file_name: String,
    #[allow(dead_code)]
    source_text: String,
    statements: Vec<ParsedStatement>,
    parser_errors: Vec<String>,
    is_module: bool,
}

pub fn check_program(files: Vec<SourceFileInput>) -> Vec<Diagnostic> {
    check_program_with_options(files, CheckerOptions::default())
}

pub fn check_program_with_options(
    files: Vec<SourceFileInput>,
    options: CheckerOptions,
) -> Vec<Diagnostic> {
    if files.is_empty() {
        return Vec::new();
    }

    let parsed_files = parse_program_files(files);
    let first_file_name = parsed_files
        .first()
        .map(|file| file.file_name.clone())
        .unwrap_or_default();
    let mut ctx = CheckerContext::new(first_file_name, options);
    let mut global_symbols = SymbolTable::new();
    let mut function_signatures = HashMap::new();

    emit_parser_diagnostics(&parsed_files, &mut ctx);
    collect_global_type_declarations(&parsed_files, &mut ctx);
    let global_type_declarations = ctx.type_declarations.clone();
    collect_global_function_signatures(
        &parsed_files,
        &mut global_symbols,
        &mut function_signatures,
        &mut ctx,
    );
    check_program_files(
        &parsed_files,
        &global_type_declarations,
        &global_symbols,
        &function_signatures,
        &mut ctx,
    );

    ctx.finish()
}

fn parse_program_files(files: Vec<SourceFileInput>) -> Vec<ParsedProgramFile> {
    files
        .into_iter()
        .map(|input| {
            let parsed = parse_source(&input.source_text, &input.file_name);
            ParsedProgramFile {
                file_name: parsed.file_name,
                source_text: input.source_text,
                statements: parsed.statements,
                parser_errors: parsed.parser_errors,
                is_module: parsed.is_module,
            }
        })
        .collect()
}

fn emit_parser_diagnostics(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());

        for message in &parsed_file.parser_errors {
            ctx.push(Diagnostic::new(
                DiagnosticCode::Custom("typescript-rust::parser-error"),
                message.clone(),
                parsed_file.file_name.clone(),
            ));
        }
    }
}

fn collect_global_type_declarations(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        if parsed_file.is_module {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        collect_type_declarations(&parsed_file.statements, ctx);
    }
}

fn collect_global_function_signatures(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if parsed_file.is_module {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());

        collect_function_signatures_from_statements(
            &parsed_file.statements,
            file_index,
            global_symbols,
            function_signatures,
            ctx,
        );
    }
}

fn check_program_files(
    parsed_files: &[ParsedProgramFile],
    global_type_declarations: &TypeDeclarationTable,
    global_symbols: &SymbolTable,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        ctx.set_file_name(parsed_file.file_name.clone());

        if parsed_file.is_module {
            let saved_type_declarations =
                std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
            let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());
            let mut local_symbols = SymbolTable::new();
            let mut local_function_signatures = HashMap::new();

            collect_type_declarations(&parsed_file.statements, ctx);
            collect_function_signatures_from_statements(
                &parsed_file.statements,
                file_index,
                &mut local_symbols,
                &mut local_function_signatures,
                ctx,
            );

            ctx.set_symbols(local_symbols);
            check_program_file_statements(
                &parsed_file.statements,
                file_index,
                &local_function_signatures,
                ctx,
            );

            ctx.type_declarations = saved_type_declarations;
            ctx.symbols = saved_symbols;
        } else {
            ctx.type_declarations = global_type_declarations.clone();
            ctx.set_symbols(global_symbols.clone());

            check_program_file_statements(
                &parsed_file.statements,
                file_index,
                function_signatures,
                ctx,
            );
        }
    }
}

fn collect_function_signatures_from_statements(
    statements: &[ParsedStatement],
    file_index: usize,
    symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (statement_index, statement) in statements.iter().enumerate() {
        collect_function_signature_from_statement(
            statement,
            file_index,
            statement_index,
            symbols,
            function_signatures,
            ctx,
        );
    }
}

fn collect_function_signature_from_statement(
    statement: &ParsedStatement,
    file_index: usize,
    statement_index: usize,
    symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::FunctionDeclaration(function) => {
            let function_type =
                check_function::collect_function_declaration_signature(function, symbols, ctx);
            function_signatures.insert(
                FunctionDeclarationLocation {
                    file_index,
                    statement_index,
                },
                function_type,
            );
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_function_signature_from_statement(
            declaration.as_ref(),
            file_index,
            statement_index,
            symbols,
            function_signatures,
            ctx,
        ),
        _ => {}
    }
}

fn check_program_file_statements(
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

fn check_program_statement(
    statement: ParsedStatement,
    file_index: usize,
    statement_index: usize,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            var::check_variable_declaration(variable, ctx);
        }
        ParsedStatement::Assignment(assignment) => {
            assign::check_assignment(assignment, ctx);
        }
        ParsedStatement::FunctionDeclaration(function) => {
            check_program_function_declaration(
                function,
                file_index,
                statement_index,
                function_signatures,
                ctx,
            );
        }
        ParsedStatement::Call(call) => {
            call::check_call(call, ctx);
        }
        ParsedStatement::Expression(expression) => {
            expr::check_expression_statement(expression, ctx);
        }
        ParsedStatement::TypeAliasDeclaration(_) => {}
        ParsedStatement::InterfaceDeclaration(_) => {}
        ParsedStatement::ImportDeclaration(_) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => check_program_statement(
            *declaration,
            file_index,
            statement_index,
            function_signatures,
            ctx,
        ),
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Empty { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { .. }) => {}
    }
}

fn check_program_function_declaration(
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

    let Some(function_type) = function_signatures.get(&declaration_location).cloned() else {
        check_function::check_function_declaration(function, ctx);
        return;
    };

    check_function::check_function_declaration_body(function, &function_type, ctx);
}
