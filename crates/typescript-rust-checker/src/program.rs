use std::collections::HashMap;
use std::rc::Rc;

use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode};
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedFunctionDeclaration,
    ParsedStatement, parse_source,
};
use typescript_rust_types::FunctionType;

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::{CheckerContext, CheckerOptions};
use crate::driver::collect_type_declarations;
use crate::modules::{
    ModuleExportTable, ModuleImportBindings, build_module_export_table,
    resolve_module_export_tables, resolve_module_imports,
};
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
pub(crate) struct ParsedProgramFile {
    pub(crate) file_name: String,
    #[allow(dead_code)]
    pub(crate) source_text: String,
    pub(crate) statements: Vec<ParsedStatement>,
    pub(crate) parser_errors: Vec<String>,
    pub(crate) is_module: bool,
}

#[derive(Debug, Clone)]
struct ModuleAnalysis {
    local_type_declarations: TypeDeclarationTable,
    local_symbols: SymbolTable,
    local_function_signatures: HashMap<FunctionDeclarationLocation, FunctionType>,
    local_export_table: ModuleExportTable,
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
    let module_analyses = collect_module_analyses(&parsed_files, &mut ctx);
    let local_module_export_tables = module_analyses
        .iter()
        .map(|analysis| {
            analysis
                .as_ref()
                .map(|analysis| analysis.local_export_table.clone())
        })
        .collect::<Vec<_>>();
    let module_export_tables =
        resolve_module_export_tables(&parsed_files, &local_module_export_tables, &mut ctx);
    let module_resolution_scopes = module_analyses
        .iter()
        .map(|analysis| {
            analysis
                .as_ref()
                .map(|analysis| Rc::new(analysis.local_type_declarations.clone()))
        })
        .collect::<Vec<_>>();
    let module_import_bindings = collect_module_import_bindings(
        &parsed_files,
        &module_analyses,
        &module_export_tables,
        &module_resolution_scopes,
        &mut ctx,
    );
    check_program_files(
        &parsed_files,
        &global_type_declarations,
        &global_symbols,
        &function_signatures,
        &module_analyses,
        &module_import_bindings,
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

fn collect_module_analyses(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleAnalysis>> {
    let mut analyses = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module {
            analyses.push(None);
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());

        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());

        collect_type_declarations(&parsed_file.statements, ctx);
        let local_type_declarations = ctx.type_declarations.clone();

        let mut local_symbols = SymbolTable::new();
        let mut local_function_signatures = HashMap::new();
        collect_function_signatures_from_statements(
            &parsed_file.statements,
            file_index,
            &mut local_symbols,
            &mut local_function_signatures,
            ctx,
        );

        let export_table =
            build_module_export_table(parsed_file, &local_type_declarations, &local_symbols, ctx);

        ctx.type_declarations = saved_type_declarations;
        ctx.symbols = saved_symbols;

        analyses.push(Some(ModuleAnalysis {
            local_type_declarations,
            local_symbols,
            local_function_signatures,
            local_export_table: export_table,
        }));
    }

    analyses
}

fn check_program_files(
    parsed_files: &[ParsedProgramFile],
    global_type_declarations: &TypeDeclarationTable,
    global_symbols: &SymbolTable,
    function_signatures: &HashMap<FunctionDeclarationLocation, FunctionType>,
    module_analyses: &[Option<ModuleAnalysis>],
    module_import_bindings: &[Option<ModuleImportBindings>],
    ctx: &mut CheckerContext,
) {
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        ctx.set_file_name(parsed_file.file_name.clone());

        if parsed_file.is_module {
            let Some(module_analysis) = module_analyses[file_index].as_ref() else {
                continue;
            };

            let imported_bindings = module_import_bindings[file_index]
                .clone()
                .unwrap_or_default();

            let mut merged_type_declarations = module_analysis.local_type_declarations.clone();
            for (name, declaration) in imported_bindings.type_declarations.iter() {
                let _ = merged_type_declarations.insert(name.clone(), declaration.clone());
            }

            let mut merged_symbols = module_analysis.local_symbols.clone();
            for (name, symbol) in imported_bindings.symbols.iter() {
                if merged_symbols.get(name).is_none() {
                    merged_symbols.insert(name.clone(), symbol.clone());
                }
            }

            ctx.type_declarations = merged_type_declarations;
            ctx.set_symbols(merged_symbols);
            check_program_file_statements(
                &parsed_file.statements,
                file_index,
                &module_analysis.local_function_signatures,
                ctx,
            );
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

fn collect_module_import_bindings(
    parsed_files: &[ParsedProgramFile],
    module_analyses: &[Option<ModuleAnalysis>],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Rc<TypeDeclarationTable>>],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleImportBindings>> {
    let mut module_import_bindings = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module {
            module_import_bindings.push(None);
            continue;
        }

        let Some(module_analysis) = module_analyses[file_index].as_ref() else {
            module_import_bindings.push(None);
            continue;
        };

        ctx.set_file_name(parsed_file.file_name.clone());
        let imported_bindings = resolve_module_imports(
            parsed_file,
            parsed_files,
            module_export_tables,
            module_resolution_scopes,
            &module_analysis.local_symbols,
            ctx,
        );
        module_import_bindings.push(Some(imported_bindings));
    }

    module_import_bindings
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
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration: ParsedDefaultExportDeclaration::Function(function),
            ..
        }) => {
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
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default { .. }) => {}
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
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration,
            ..
        }) => match declaration {
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
            ParsedDefaultExportDeclaration::Unsupported { span } => {
                let mut diagnostic = Diagnostic::new(
                    DiagnosticCode::Custom("typescript-rust::unsupported-module-syntax"),
                    "Unsupported module syntax.".to_string(),
                    ctx.file_name.clone(),
                );

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        },
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Empty { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { span }) => {
            let mut diagnostic = Diagnostic::new(
                DiagnosticCode::Custom("typescript-rust::unsupported-module-syntax"),
                "Unsupported module syntax.".to_string(),
                ctx.file_name.clone(),
            );

            if let Some(span) = span {
                diagnostic = diagnostic.with_span(crate::context::convert_span(span));
            }

            ctx.push(diagnostic);
        }
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
