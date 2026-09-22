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
        ctx.merge_script_interfaces_with_globals = true;
        collect_type_declarations(&parsed_file.statements, ctx);
        ctx.merge_script_interfaces_with_globals = false;
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
    let seeded_names = seed_script_values_for_signatures(parsed_files, global_symbols, ctx);
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !is_script_source(parsed_file) {
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
    for (name, seeded) in seeded_names {
        if global_symbols
            .get_own(&name)
            .is_some_and(|symbol| std::ptr::eq(symbol, seeded.as_ref()))
        {
            global_symbols.remove(&name);
        }
    }
}

fn is_script_source(parsed_file: &ParsedProgramFile) -> bool {
    parsed_file.file_kind != FileKind::GeneratedDeclaration
        && !parsed_file.is_module
        && !parsed_file.file_kind.is_declaration()
}

/// A script's top-level `var`s are globals the binder declares before any
/// signature is resolved, so `function f(x: typeof a)` reads `a` wherever it
/// is written. Signature collection runs before the check phase declares
/// them; seed their values for it, as module analysis does for a module's
/// locals. The seed is removed afterwards so the check phase declares each
/// variable itself.
fn seed_script_values_for_signatures(
    parsed_files: &[ParsedProgramFile],
    global_symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) -> Vec<(Arc<str>, crate::symbols::SymbolInfoHandle)> {
    let script_sources = || parsed_files.iter().filter(|parsed_file| is_script_source(parsed_file));
    if !script_sources().any(|parsed_file| parsed_file.contains_typeof) {
        return Vec::new();
    }
    let mut seeded = Vec::new();
    for parsed_file in script_sources() {
        ctx.set_file_name(parsed_file.file_name.clone());
        let values = crate::modules::collect_exportable_value_symbols(
            &parsed_file.statements,
            &ctx.type_declarations,
            &SymbolTable::new(),
            None,
            false,
            ctx,
        );
        for (name, symbol) in values.iter_shared() {
            if global_symbols.get(name).is_none() {
                let _ = global_symbols.insert_shared(name.clone(), symbol.clone());
                seeded.push((name.clone(), symbol.clone()));
            }
        }
    }
    seeded
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
    let outer_collecting_signatures = std::mem::replace(&mut ctx.collecting_signatures, true);
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
    // Expando members are hoisted with the function they are written on, so a
    // function declared earlier in the file can already read them.
    crate::modules::exports::apply_expando_members(statements, symbols, ctx);
    ctx.collecting_signatures = outer_collecting_signatures;
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
            let symbol = super::build_class_value_symbol_with_scope(class, Some(symbols), ctx);
            symbols.insert(class.name.clone(), symbol);
        }
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Default {
                declaration: ParsedDefaultExportDeclaration::Class(class),
                ..
            } => {
                let symbol = super::build_class_value_symbol_with_scope(class, Some(symbols), ctx);
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
                    // `let assert: typeof import("vitest")["assert"]` reads a
                    // module namespace that only exists once imports are bound,
                    // after this pass; the annotation maps on first read instead.
                    let ty = match var.declared_type.as_ref() {
                        Some(annotation)
                            if crate::modules::exports::annotation_contains_import_type_query(
                                annotation,
                            ) =>
                        {
                            {
                            ctx.register_import_type_global(&var.name);
                            crate::infer::make_lazy_value_annotation_reference(
                                ctx,
                                &var.name,
                                var.name_span.map_or(0, |span| span.start),
                                annotation.clone(),
                            )
                        }
                        }
                        Some(annotation) => crate::infer::map_parsed_type(annotation.clone(), ctx),
                        None => surge_ts_types::Type::Unknown,
                    };
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
                        if !matches!(
                            inferred_ty,
                            surge_ts_types::Type::Unknown
                                | surge_ts_types::Type::TypeParameter(_)
                        ) =>
                    {
                        var::widen_implicit_variable_initializer_type(
                            symbol_kind,
                            initializer,
                            &inferred_ty,
                            var::is_auto_array_candidate(var, ctx),
                        )
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
