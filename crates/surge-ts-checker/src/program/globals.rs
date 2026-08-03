//! Script-global type, function-signature, and value-symbol collection.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use surge_ts_syntax::{ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedStatement};
use surge_ts_types::FunctionType;

use super::*;

use crate::checks::{expr, function as check_function, var};
use crate::context::{CheckerContext, FileKind};
use crate::driver::collect_type_declarations;
use crate::symbols::SymbolTable;

pub(crate) fn collect_global_type_declarations(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) {
    for parsed_file in parsed_files {
        if parsed_file.file_kind == FileKind::GeneratedDeclaration {
            continue;
        }

        if parsed_file.file_kind == FileKind::DependencyDeclaration || parsed_file.is_module {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let collect_start = Instant::now();
        collect_type_declarations(&parsed_file.statements, ctx);
        let lowered_type_declarations = ctx.type_declarations.len() as u64;
        let collect_duration = collect_start.elapsed();
        record_program_file_timing(timings, &parsed_file.file_name, |timings| {
            timings.collect_type_declarations_passes += 1;
            timings.lowered_type_declarations += lowered_type_declarations;
            timings.collect_type_declarations_duration += collect_duration
        });
    }
}

pub(crate) fn collect_global_function_signatures(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if parsed_file.file_kind == FileKind::GeneratedDeclaration {
            continue;
        }

        if parsed_file.is_module || parsed_file.file_kind.is_declaration() {
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

pub(crate) fn collect_function_signatures_from_statements(
    statements: &[ParsedStatement],
    file_index: usize,
    symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
) {
    let mut declaration_counts = HashMap::<String, usize>::new();
    for statement in statements {
        count_function_declarations(statement, &mut declaration_counts);
    }
    for (statement_index, statement) in statements.iter().enumerate() {
        collect_function_signature_from_statement(
            statement,
            file_index,
            statement_index,
            symbols,
            function_signatures,
            ctx,
            &declaration_counts,
        );
    }
}

fn count_function_declarations(
    statement: &ParsedStatement,
    declaration_counts: &mut HashMap<String, usize>,
) {
    match statement {
        ParsedStatement::FunctionDeclaration(function) => {
            *declaration_counts.entry(function.name.clone()).or_default() += 1;
        }
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                count_function_declarations(declaration, declaration_counts)
            }
            ParsedExportDeclaration::Default {
                declaration: ParsedDefaultExportDeclaration::Function(function),
                ..
            } => {
                *declaration_counts.entry(function.name.clone()).or_default() += 1;
            }
            _ => {}
        },
        _ => {}
    }
}

pub(crate) fn collect_function_signature_from_statement(
    statement: &ParsedStatement,
    file_index: usize,
    statement_index: usize,
    symbols: &mut SymbolTable,
    function_signatures: &mut HashMap<FunctionDeclarationLocation, FunctionType>,
    ctx: &mut CheckerContext,
    declaration_counts: &HashMap<String, usize>,
) {
    match statement {
        ParsedStatement::FunctionDeclaration(function) => {
            let function_type = check_function::collect_function_declaration_signature(
                function,
                symbols,
                ctx,
                declaration_counts.get(&function.name) == Some(&1),
            );
            function_signatures.insert(
                FunctionDeclarationLocation {
                    file_index,
                    statement_index,
                },
                function_type,
            );
        }
        ParsedStatement::ClassDeclaration(class) => {
            let symbol = super::build_class_value_symbol(class, ctx);
            symbols.insert(class.name.clone(), symbol);
        }
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Default {
                declaration: ParsedDefaultExportDeclaration::Class(class),
                ..
            } => {
                let symbol = super::build_class_value_symbol(class, ctx);
                symbols.insert(class.name.clone(), symbol);
            }
            ParsedExportDeclaration::Statement { declaration, .. } => {
                collect_function_signature_from_statement(
                    declaration.as_ref(),
                    file_index,
                    statement_index,
                    symbols,
                    function_signatures,
                    ctx,
                    declaration_counts,
                )
            }
            ParsedExportDeclaration::Default {
                declaration: ParsedDefaultExportDeclaration::Function(function),
                ..
            } => {
                let function_type = check_function::collect_function_declaration_signature(
                    function,
                    symbols,
                    ctx,
                    declaration_counts.get(&function.name) == Some(&1),
                );
                function_signatures.insert(
                    FunctionDeclarationLocation {
                        file_index,
                        statement_index,
                    },
                    function_type,
                );
            }
            ParsedExportDeclaration::Default { .. } => {}
            _ => {}
        },
        _ => {}
    }
}

pub(crate) fn collect_global_variables(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    // Annotations are mapped here, long before `global_symbols` becomes the
    // check phase's symbol table, so a `typeof <scriptGlobal>` query resolves
    // against a context that cannot see the script globals collected above it —
    // class constructor symbols in particular, which exist nowhere else at this
    // point. Expose the snapshot as the `typeof` fallback so
    // `declare const x: typeof SomeClass` resolves instead of reporting a false
    // TS2304. Purely additive: the fallback is the last link of the lookup
    // chain, and the loop only ever adds variables to `global_symbols`.
    let saved_module_value_fallback = ctx.module_value_fallback.replace(Arc::new(
        global_symbols.clone_with_reason(surge_ts_types::TypeCopyReason::SymbolTable),
    ));

    for parsed_file in parsed_files {
        if parsed_file.file_kind == FileKind::GeneratedDeclaration {
            continue;
        }

        if parsed_file.file_kind == FileKind::DependencyDeclaration || parsed_file.is_module {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        for statement in &parsed_file.statements {
            let var = match statement {
                ParsedStatement::VariableDeclaration(var) => Some(var),
                ParsedStatement::ExportDeclaration(export) => {
                    if let surge_ts_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    } = export.as_ref()
                    {
                        if let ParsedStatement::VariableDeclaration(var) = declaration.as_ref() {
                            Some(var)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(var) = var {
                if var.is_declare || parsed_file.file_kind.is_declaration() {
                    let ty = var
                        .declared_type
                        .as_ref()
                        .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                        .unwrap_or(surge_ts_types::Type::Unknown);
                    if !parsed_file.file_kind.is_declaration()
                        || global_symbols.get(&var.name).is_none()
                    {
                        global_symbols.insert(
                            var.name.clone(),
                            crate::symbols::SymbolInfo {
                                kind: if matches!(
                                    var.kind,
                                    surge_ts_syntax::ParsedVariableKind::Const
                                ) {
                                    crate::symbols::SymbolKind::Const
                                } else {
                                    crate::symbols::SymbolKind::Let
                                },
                                ty,
                                function_signature: None,
                            },
                        );
                    }
                }
            }
        }
    }

    ctx.module_value_fallback = saved_module_value_fallback;
}

#[allow(dead_code)]
#[allow(dead_code)]
pub(crate) fn collect_local_value_symbols(
    statements: &[ParsedStatement],
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        collect_local_value_symbols_from_statement(statement, symbols, ctx);
    }
}

#[allow(dead_code)]
#[allow(dead_code)]
pub(crate) fn collect_local_value_symbols_from_statement(
    statement: &ParsedStatement,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(var) => {
            if var.is_declare {
                return;
            }

            let symbol_kind = if matches!(var.kind, surge_ts_syntax::ParsedVariableKind::Const) {
                crate::symbols::SymbolKind::Const
            } else {
                crate::symbols::SymbolKind::Let
            };

            let ty = if let Some(declared_type) = var.declared_type.as_ref() {
                crate::infer::map_parsed_type(declared_type.clone(), ctx)
            } else if let Some(initializer) = var.initializer.as_ref() {
                let inferred =
                    expr::evaluate_expression(initializer, var.initializer_span, symbols, ctx);

                match inferred {
                    crate::infer::InferredExpression::Known(inferred_ty)
                        if inferred_ty != surge_ts_types::Type::Unknown =>
                    {
                        var::widen_implicit_variable_initializer_type(symbol_kind, &inferred_ty)
                    }
                    _ => surge_ts_types::Type::Unknown,
                }
            } else {
                surge_ts_types::Type::Unknown
            };

            symbols.insert(
                var.name.clone(),
                crate::symbols::SymbolInfo {
                    ty,
                    kind: symbol_kind,
                    function_signature: None,
                },
            );
        }
        ParsedStatement::ExportDeclaration(export) => {
            if let surge_ts_syntax::ParsedExportDeclaration::Statement { declaration, .. } =
                export.as_ref()
            {
                collect_local_value_symbols_from_statement(declaration.as_ref(), symbols, ctx)
            }
        }
        _ => {}
    }
}
