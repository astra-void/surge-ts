use std::collections::HashMap;
use std::rc::Rc;

use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode};
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedImportDeclaration,
    ParsedImportKind, ParsedStatement, ParsedType, TextSpan,
};

use crate::context::{CheckerContext, convert_span};
use crate::program::ParsedProgramFile;
use crate::symbols::{
    SymbolInfo, SymbolKind, SymbolTable, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationTable,
};
use crate::{
    checks::var::VariableCheckOptions, checks::var::check_variable_declaration_with_symbols,
};
use typescript_rust_types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ModuleKey(String);

#[derive(Debug, Clone)]
pub(crate) struct ModuleResolution {
    pub(crate) resolved_file_index: usize,
    #[allow(dead_code)]
    pub(crate) resolved_file_name: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleExportTable {
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) symbols: SymbolTable,
    pub(crate) default_symbol: Option<SymbolInfo>,
    pub(crate) has_unresolved_star_export: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleImportBindings {
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) symbols: SymbolTable,
}

pub(crate) fn is_relative_specifier(specifier: &str) -> bool {
    specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with(".\\")
        || specifier.starts_with("..\\")
}

pub(crate) fn is_external_specifier(specifier: &str) -> bool {
    !is_relative_specifier(specifier)
}

pub(crate) fn resolve_relative_module(
    importer_file_name: &str,
    specifier: &str,
    program_files: &[ParsedProgramFile],
) -> Option<ModuleResolution> {
    if !is_relative_specifier(specifier) {
        return None;
    }

    let file_index_by_key = program_files
        .iter()
        .enumerate()
        .map(|(index, file)| (ModuleKey(normalize_module_path(&file.file_name)), index))
        .collect::<HashMap<_, _>>();

    let importer_dir = module_directory(importer_file_name);
    let normalized_specifier = normalize_module_path(specifier);
    let joined_specifier = if importer_dir.is_empty() {
        normalized_specifier.clone()
    } else {
        normalize_module_path(&format!("{importer_dir}/{normalized_specifier}"))
    };

    let candidate_paths = match relative_specifier_kind(&normalized_specifier) {
        RelativeSpecifierKind::ExplicitTs => vec![joined_specifier],
        RelativeSpecifierKind::Extensionless => vec![
            format!("{joined_specifier}.ts"),
            format!("{joined_specifier}/index.ts"),
        ],
        RelativeSpecifierKind::Unsupported => return None,
    };

    for candidate in candidate_paths {
        if let Some(resolved_file_index) = file_index_by_key.get(&ModuleKey(candidate.clone())) {
            return Some(ModuleResolution {
                resolved_file_index: *resolved_file_index,
                resolved_file_name: program_files[*resolved_file_index].file_name.clone(),
            });
        }
    }

    None
}

pub(crate) fn build_module_export_table(
    parsed_file: &ParsedProgramFile,
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> ModuleExportTable {
    let exportable_values = collect_exportable_value_symbols(
        &parsed_file.statements,
        local_type_declarations,
        local_symbols,
        ctx,
    );

    let mut type_declarations = TypeDeclarationTable::new();
    let mut symbols = SymbolTable::new();
    let mut default_symbol = None;

    for statement in &parsed_file.statements {
        collect_exports_from_statement(
            statement,
            &exportable_values,
            local_type_declarations,
            local_symbols,
            &mut type_declarations,
            &mut symbols,
            &mut default_symbol,
            ctx,
        );
    }

    ModuleExportTable {
        type_declarations,
        symbols,
        default_symbol,
        has_unresolved_star_export: false,
    }
}

pub(crate) fn resolve_module_export_tables(
    parsed_files: &[ParsedProgramFile],
    local_module_export_tables: &[Option<ModuleExportTable>],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleExportTable>> {
    let mut resolved_module_export_tables = vec![None; parsed_files.len()];
    let mut resolving = vec![false; parsed_files.len()];

    for file_index in 0..parsed_files.len() {
        let _ = resolve_module_export_table(
            file_index,
            parsed_files,
            local_module_export_tables,
            &mut resolved_module_export_tables,
            &mut resolving,
            ctx,
        );
    }

    resolved_module_export_tables
}

pub(crate) fn resolve_module_imports(
    parsed_file: &ParsedProgramFile,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Rc<TypeDeclarationTable>>],
    local_symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> ModuleImportBindings {
    let mut type_declarations = TypeDeclarationTable::new();
    let mut symbols = SymbolTable::new();

    for statement in &parsed_file.statements {
        let ParsedStatement::ImportDeclaration(import) = statement else {
            continue;
        };

        resolve_import_declaration(
            import,
            program_files,
            module_export_tables,
            module_resolution_scopes,
            local_symbols,
            &mut type_declarations,
            &mut symbols,
            ctx,
        );
    }

    ModuleImportBindings {
        type_declarations,
        symbols,
    }
}

fn resolve_module_export_table(
    file_index: usize,
    parsed_files: &[ParsedProgramFile],
    local_module_export_tables: &[Option<ModuleExportTable>],
    resolved_module_export_tables: &mut [Option<ModuleExportTable>],
    resolving: &mut [bool],
    ctx: &mut CheckerContext,
) -> Option<ModuleExportTable> {
    if let Some(resolved) = resolved_module_export_tables[file_index].clone() {
        return Some(resolved);
    }

    let Some(local_export_table) = local_module_export_tables[file_index].clone() else {
        return None;
    };

    if resolving[file_index] {
        return Some(local_export_table);
    }

    resolving[file_index] = true;
    ctx.set_file_name(parsed_files[file_index].file_name.clone());

    let mut resolved_export_table = local_export_table;

    for statement in &parsed_files[file_index].statements {
        match statement {
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
                is_type_only,
                specifiers,
                module_specifier: Some(module_specifier),
                module_specifier_span,
                ..
            }) => {
                let Some(resolved_module) = resolve_relative_module(
                    &parsed_files[file_index].file_name,
                    module_specifier,
                    parsed_files,
                ) else {
                    if !(ctx.options.stub_external_modules
                        && is_external_specifier(module_specifier))
                    {
                        emit_unresolved_export_module_diagnostic(
                            ctx,
                            module_specifier,
                            *module_specifier_span,
                        );
                    }

                    for specifier in specifiers {
                        if *is_type_only {
                            insert_unknown_type_import(
                                &mut resolved_export_table.type_declarations,
                                &specifier.exported_name,
                                ctx.file_name.clone(),
                                specifier.name_span,
                            );
                            continue;
                        }

                        insert_unknown_type_import(
                            &mut resolved_export_table.type_declarations,
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        insert_unknown_value_import(
                            &specifier.exported_name,
                            &mut resolved_export_table.symbols,
                        );
                    }

                    continue;
                };

                let Some(target_export_table) = resolve_module_export_table(
                    resolved_module.resolved_file_index,
                    parsed_files,
                    local_module_export_tables,
                    resolved_module_export_tables,
                    resolving,
                    ctx,
                ) else {
                    ctx.set_file_name(parsed_files[file_index].file_name.clone());

                    for specifier in specifiers {
                        emit_missing_export_diagnostic(
                            ctx,
                            module_specifier,
                            &specifier.local_name,
                            specifier.name_span,
                        );

                        if *is_type_only {
                            insert_unknown_type_import(
                                &mut resolved_export_table.type_declarations,
                                &specifier.exported_name,
                                ctx.file_name.clone(),
                                specifier.name_span,
                            );
                            continue;
                        }

                        insert_unknown_type_import(
                            &mut resolved_export_table.type_declarations,
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        insert_unknown_value_import(
                            &specifier.exported_name,
                            &mut resolved_export_table.symbols,
                        );
                    }

                    continue;
                };

                ctx.set_file_name(parsed_files[file_index].file_name.clone());

                for specifier in specifiers {
                    let type_export = target_export_table
                        .type_declarations
                        .get(&specifier.local_name)
                        .cloned();
                    let value_export = target_export_table
                        .symbols
                        .get(&specifier.local_name)
                        .cloned();

                    if *is_type_only {
                        if let Some(type_export) = type_export {
                            export_local_type_declaration(
                                &type_export,
                                &specifier.exported_name,
                                &mut resolved_export_table.type_declarations,
                            );
                            continue;
                        }

                        emit_missing_export_diagnostic(
                            ctx,
                            module_specifier,
                            &specifier.local_name,
                            specifier.name_span,
                        );
                        insert_unknown_type_import(
                            &mut resolved_export_table.type_declarations,
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        continue;
                    }

                    let mut found = false;

                    if let Some(type_export) = type_export {
                        export_local_type_declaration(
                            &type_export,
                            &specifier.exported_name,
                            &mut resolved_export_table.type_declarations,
                        );
                        found = true;
                    }

                    if let Some(value_export) = value_export {
                        let _ = resolved_export_table
                            .symbols
                            .insert(specifier.exported_name.clone(), value_export);
                        found = true;
                    }

                    if !found {
                        if target_export_table.has_unresolved_star_export
                            || module_has_unresolved_star_export(
                                resolved_module.resolved_file_index,
                                parsed_files,
                            )
                        {
                            insert_unknown_type_import(
                                &mut resolved_export_table.type_declarations,
                                &specifier.exported_name,
                                ctx.file_name.clone(),
                                specifier.name_span,
                            );
                            insert_unknown_value_import(
                                &specifier.exported_name,
                                &mut resolved_export_table.symbols,
                            );
                            continue;
                        }

                        emit_missing_export_diagnostic(
                            ctx,
                            module_specifier,
                            &specifier.local_name,
                            specifier.name_span,
                        );
                        insert_unknown_type_import(
                            &mut resolved_export_table.type_declarations,
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        insert_unknown_value_import(
                            &specifier.exported_name,
                            &mut resolved_export_table.symbols,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    for statement in &parsed_files[file_index].statements {
        let ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
            module_specifier,
            module_specifier_span,
            ..
        }) = statement
        else {
            continue;
        };

        let Some(resolved_module) = resolve_relative_module(
            &parsed_files[file_index].file_name,
            module_specifier,
            parsed_files,
        ) else {
            if !(ctx.options.stub_external_modules && is_external_specifier(module_specifier)) {
                emit_unresolved_export_module_diagnostic(
                    ctx,
                    module_specifier,
                    *module_specifier_span,
                );
            }
            resolved_export_table.has_unresolved_star_export = true;
            continue;
        };

        let Some(target_export_table) = resolve_module_export_table(
            resolved_module.resolved_file_index,
            parsed_files,
            local_module_export_tables,
            resolved_module_export_tables,
            resolving,
            ctx,
        ) else {
            ctx.set_file_name(parsed_files[file_index].file_name.clone());
            continue;
        };

        ctx.set_file_name(parsed_files[file_index].file_name.clone());

        for (name, declaration) in target_export_table.type_declarations.iter() {
            if resolved_export_table.type_declarations.get(name).is_none() {
                let _ = resolved_export_table
                    .type_declarations
                    .insert(name.clone(), declaration.clone());
            }
        }

        for (name, symbol) in target_export_table.symbols.iter() {
            if resolved_export_table.symbols.get(name).is_none() {
                resolved_export_table
                    .symbols
                    .insert(name.clone(), symbol.clone());
            }
        }
    }

    resolving[file_index] = false;
    resolved_module_export_tables[file_index] = Some(resolved_export_table.clone());
    Some(resolved_export_table)
}

fn collect_exportable_value_symbols(
    statements: &[ParsedStatement],
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> SymbolTable {
    let mut shadow_ctx = CheckerContext::new(ctx.file_name.clone(), ctx.options);
    shadow_ctx.type_declarations = local_type_declarations.clone();

    let mut exportable_values = local_symbols.clone();

    for statement in statements {
        collect_exportable_value_symbols_from_statement(
            statement,
            &mut exportable_values,
            &mut shadow_ctx,
        );
    }

    exportable_values
}

fn collect_exportable_value_symbols_from_statement(
    statement: &ParsedStatement,
    exportable_values: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            let _ = check_variable_declaration_with_symbols(
                variable.clone(),
                exportable_values,
                ctx,
                VariableCheckOptions {
                    report_duplicate_let_const: false,
                    check_initializer: true,
                },
            );
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_exportable_value_symbols_from_statement(
            declaration.as_ref(),
            exportable_values,
            ctx,
        ),
        _ => {}
    }
}

fn collect_exports_from_statement(
    statement: &ParsedStatement,
    exportable_values: &SymbolTable,
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    default_symbol: &mut Option<SymbolInfo>,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_exports_from_statement(
            declaration.as_ref(),
            exportable_values,
            local_type_declarations,
            local_symbols,
            type_declarations,
            symbols,
            default_symbol,
            ctx,
        ),
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
            is_type_only,
            specifiers,
            module_specifier,
            ..
        }) => {
            if module_specifier.is_some() {
                return;
            }

            for specifier in specifiers {
                if *is_type_only {
                    export_local_type_name(
                        &specifier.local_name,
                        &specifier.exported_name,
                        &specifier.name_span,
                        local_type_declarations,
                        type_declarations,
                        ctx,
                    );
                    continue;
                }

                let mut found = false;

                if let Some(type_declaration) = local_type_declarations.get(&specifier.local_name) {
                    export_local_type_declaration(
                        type_declaration,
                        &specifier.exported_name,
                        type_declarations,
                    );
                    found = true;
                }

                if let Some(symbol) = exportable_values.get(&specifier.local_name) {
                    symbols.insert(specifier.exported_name.clone(), symbol.clone());
                    found = true;
                }

                if !found {
                    push_unresolved_export_diagnostic(
                        ctx,
                        &specifier.local_name,
                        specifier.name_span,
                    );
                }
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration,
            span,
        }) => match declaration {
            ParsedDefaultExportDeclaration::Function(function) => {
                if let Some(symbol) = local_symbols.get(&function.name) {
                    if default_symbol.is_some() {
                        push_duplicate_default_export_diagnostic(ctx, function.name_span.or(*span));
                    } else {
                        *default_symbol = Some(symbol.clone());
                    }
                } else {
                    push_duplicate_default_export_diagnostic(ctx, function.name_span.or(*span));
                }
            }
            ParsedDefaultExportDeclaration::Expression(expression) => {
                if default_symbol.is_some() {
                    push_duplicate_default_export_diagnostic(ctx, *span);
                    return;
                }

                let ty = crate::infer::infer_expression(expression, exportable_values);
                let ty = match ty {
                    crate::infer::InferredExpression::Known(ty) => ty,
                    crate::infer::InferredExpression::Unknown
                    | crate::infer::InferredExpression::UnresolvedIdentifier { .. }
                    | crate::infer::InferredExpression::MissingProperty { .. } => Type::Unknown,
                };

                *default_symbol = Some(SymbolInfo {
                    ty,
                    kind: SymbolKind::Const,
                });
            }
            ParsedDefaultExportDeclaration::Unsupported { .. } => {}
        },
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All { .. }) => {}
        ParsedStatement::TypeAliasDeclaration(alias) => {
            export_local_type_name(
                &alias.name,
                &alias.name,
                &alias.name_span,
                local_type_declarations,
                type_declarations,
                ctx,
            );
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            export_local_type_name(
                &interface.name,
                &interface.name,
                &interface.name_span,
                local_type_declarations,
                type_declarations,
                ctx,
            );
        }
        ParsedStatement::FunctionDeclaration(function) => {
            if let Some(symbol) = local_symbols.get(&function.name) {
                symbols.insert(function.name.clone(), symbol.clone());
            }
        }
        ParsedStatement::VariableDeclaration(variable) => {
            if let Some(symbol) = exportable_values.get(&variable.name) {
                symbols.insert(variable.name.clone(), symbol.clone());
            }
        }
        _ => {}
    }
}

fn resolve_import_declaration(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Rc<TypeDeclarationTable>>],
    local_symbols: &SymbolTable,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    match &import.kind {
        ParsedImportKind::Unsupported => {
            emit_unsupported_module_syntax_diagnostic(ctx, import);
            return;
        }
        ParsedImportKind::Default {
            local_name,
            name_span,
        } => {
            let Some(resolved) =
                resolve_relative_module(&ctx.file_name, &import.module_specifier, program_files)
            else {
                if !(ctx.options.stub_external_modules
                    && is_external_specifier(&import.module_specifier))
                {
                    emit_unresolved_module_diagnostic(ctx, import);
                }
                insert_unknown_value_import(local_name, symbols);
                return;
            };

            let Some(Some(export_table)) = module_export_tables.get(resolved.resolved_file_index)
            else {
                emit_missing_export_diagnostic(
                    ctx,
                    &import.module_specifier,
                    "default",
                    *name_span,
                );
                insert_unknown_value_import(local_name, symbols);
                return;
            };

            let Some(default_symbol) = export_table.default_symbol.clone() else {
                emit_missing_export_diagnostic(
                    ctx,
                    &import.module_specifier,
                    "default",
                    *name_span,
                );
                insert_unknown_value_import(local_name, symbols);
                return;
            };

            if local_symbols.get(local_name).is_none() {
                symbols.insert(local_name.clone(), default_symbol);
            }
            return;
        }
        ParsedImportKind::Namespace {
            local_name,
            name_span: _,
        } => {
            let Some(resolved) =
                resolve_relative_module(&ctx.file_name, &import.module_specifier, program_files)
            else {
                if !(ctx.options.stub_external_modules
                    && is_external_specifier(&import.module_specifier))
                {
                    emit_unresolved_module_diagnostic(ctx, import);
                }
                insert_unknown_value_import(local_name, symbols);
                return;
            };

            let namespace_type = module_export_tables
                .get(resolved.resolved_file_index)
                .and_then(|table| table.as_ref())
                .map(namespace_export_object_type)
                .unwrap_or_else(|| {
                    Type::Object(typescript_rust_types::ObjectType {
                        properties: std::collections::BTreeMap::new(),
                    })
                });

            if local_symbols.get(local_name).is_none() {
                symbols.insert(
                    local_name.clone(),
                    SymbolInfo {
                        ty: namespace_type,
                        kind: SymbolKind::Const,
                    },
                );
            }
            return;
        }
        ParsedImportKind::SideEffect => {
            if resolve_relative_module(&ctx.file_name, &import.module_specifier, program_files)
                .is_none()
            {
                if !(ctx.options.stub_external_modules
                    && is_external_specifier(&import.module_specifier))
                {
                    emit_unresolved_module_diagnostic(ctx, import);
                }
            }
            return;
        }
        ParsedImportKind::Named {
            is_type_only,
            specifiers,
        } => {
            let Some(resolved) =
                resolve_relative_module(&ctx.file_name, &import.module_specifier, program_files)
            else {
                if !(ctx.options.stub_external_modules
                    && is_external_specifier(&import.module_specifier))
                {
                    emit_unresolved_module_diagnostic(ctx, import);
                }
                for specifier in specifiers {
                    if *is_type_only {
                        insert_unknown_type_import(
                            type_declarations,
                            &specifier.local_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        continue;
                    }

                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    insert_unknown_value_import(&specifier.local_name, symbols);
                }
                return;
            };

            let Some(scope) = module_resolution_scopes
                .get(resolved.resolved_file_index)
                .and_then(|scope| scope.clone())
            else {
                for specifier in specifiers {
                    emit_missing_export_diagnostic(
                        ctx,
                        &import.module_specifier,
                        &specifier.imported_name,
                        specifier.name_span,
                    );

                    if *is_type_only {
                        insert_unknown_type_import(
                            type_declarations,
                            &specifier.local_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        continue;
                    }

                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    insert_unknown_value_import(&specifier.local_name, symbols);
                }
                return;
            };

            let Some(Some(export_table)) = module_export_tables.get(resolved.resolved_file_index)
            else {
                for specifier in specifiers {
                    emit_missing_export_diagnostic(
                        ctx,
                        &import.module_specifier,
                        &specifier.imported_name,
                        specifier.name_span,
                    );

                    if *is_type_only {
                        insert_unknown_type_import(
                            type_declarations,
                            &specifier.local_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        continue;
                    }

                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    insert_unknown_value_import(&specifier.local_name, symbols);
                }
                return;
            };

            let has_unresolved_star_export = export_table.has_unresolved_star_export
                || module_has_unresolved_star_export(resolved.resolved_file_index, program_files);

            for specifier in specifiers {
                if has_unresolved_star_export {
                    if *is_type_only {
                        insert_unknown_type_import(
                            type_declarations,
                            &specifier.local_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        continue;
                    }

                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    insert_unknown_value_import(&specifier.local_name, symbols);
                    continue;
                }

                let type_export = export_table
                    .type_declarations
                    .get(&specifier.imported_name)
                    .cloned();
                let value_export = export_table.symbols.get(&specifier.imported_name).cloned();

                if *is_type_only {
                    if let Some(type_export) = type_export {
                        insert_type_export(
                            type_declarations,
                            &specifier.local_name,
                            attach_type_resolution_scope(type_export, scope.clone()),
                        );
                        continue;
                    }

                    emit_missing_export_diagnostic(
                        ctx,
                        &import.module_specifier,
                        &specifier.imported_name,
                        specifier.name_span,
                    );
                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    continue;
                }

                let mut found = false;

                if let Some(type_export) = type_export {
                    insert_type_export(
                        type_declarations,
                        &specifier.local_name,
                        attach_type_resolution_scope(type_export, scope.clone()),
                    );
                    found = true;
                }

                if let Some(value_export) = value_export {
                    if local_symbols.get(&specifier.local_name).is_none() {
                        symbols.insert(specifier.local_name.clone(), value_export);
                    }
                    found = true;
                }

                if !found {
                    emit_missing_export_diagnostic(
                        ctx,
                        &import.module_specifier,
                        &specifier.imported_name,
                        specifier.name_span,
                    );
                    insert_unknown_type_import(
                        type_declarations,
                        &specifier.local_name,
                        ctx.file_name.clone(),
                        specifier.name_span,
                    );
                    insert_unknown_value_import(&specifier.local_name, symbols);
                }
            }
        }
    };
}

fn export_local_type_name(
    local_name: &str,
    exported_name: &str,
    name_span: &Option<TextSpan>,
    local_type_declarations: &TypeDeclarationTable,
    type_declarations: &mut TypeDeclarationTable,
    ctx: &mut CheckerContext,
) {
    let Some(local_declaration) = local_type_declarations.get(local_name).cloned() else {
        push_unresolved_export_diagnostic(ctx, local_name, *name_span);
        return;
    };

    export_local_type_declaration(&local_declaration, exported_name, type_declarations);
}

fn export_local_type_declaration(
    declaration: &TypeDeclarationInfo,
    exported_name: &str,
    type_declarations: &mut TypeDeclarationTable,
) {
    let declaration = rename_type_declaration(declaration.clone(), exported_name.to_string());
    let _ = type_declarations.insert(exported_name.to_string(), declaration);
}

fn rename_type_declaration(
    declaration: TypeDeclarationInfo,
    exported_name: String,
) -> TypeDeclarationInfo {
    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            alias.name = exported_name;
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            interface.name = exported_name;
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

fn insert_type_export(
    type_declarations: &mut TypeDeclarationTable,
    local_name: &str,
    declaration: TypeDeclarationInfo,
) {
    let declaration = rename_type_declaration(declaration, local_name.to_string());
    let _ = type_declarations.insert(local_name.to_string(), declaration);
}

fn insert_unknown_type_import(
    type_declarations: &mut TypeDeclarationTable,
    local_name: &str,
    file_name: String,
    name_span: Option<TextSpan>,
) {
    let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
        name: local_name.to_string(),
        file_name,
        name_span,
        type_parameters: vec![],
        ty: ParsedType::Unknown,
        resolution_scope: None,
    });

    let _ = type_declarations.insert(local_name.to_string(), declaration);
}

fn insert_unknown_value_import(local_name: &str, symbols: &mut SymbolTable) {
    let _ = symbols.insert(
        local_name.to_string(),
        SymbolInfo {
            ty: Type::Unknown,
            kind: SymbolKind::Var,
        },
    );
}

fn namespace_export_object_type(export_table: &ModuleExportTable) -> Type {
    let mut properties = std::collections::BTreeMap::new();

    for (name, symbol) in export_table.symbols.iter() {
        properties.insert(
            name.clone(),
            typescript_rust_types::ObjectProperty::required(symbol.ty.clone()),
        );
    }

    if let Some(default_symbol) = &export_table.default_symbol {
        properties.insert(
            "default".to_string(),
            typescript_rust_types::ObjectProperty::required(default_symbol.ty.clone()),
        );
    }

    Type::Object(typescript_rust_types::ObjectType { properties })
}

fn push_duplicate_default_export_diagnostic(ctx: &mut CheckerContext, name_span: Option<TextSpan>) {
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::Custom("typescript-rust::duplicate-default-export"),
        "Duplicate default export.".to_string(),
        ctx.file_name.clone(),
    );

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn emit_unresolved_export_module_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    module_specifier_span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::TypeScript(2307),
        format!(
            "Cannot find module '{}' or its corresponding type declarations.",
            module_specifier
        ),
        ctx.file_name.clone(),
    );

    if let Some(span) = module_specifier_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn emit_unresolved_module_diagnostic(ctx: &mut CheckerContext, import: &ParsedImportDeclaration) {
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::TypeScript(2307),
        format!(
            "Cannot find module '{}' or its corresponding type declarations.",
            import.module_specifier
        ),
        ctx.file_name.clone(),
    );

    if let Some(span) = import.module_specifier_span.or(import.span) {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn module_has_unresolved_star_export(
    file_index: usize,
    parsed_files: &[ParsedProgramFile],
) -> bool {
    parsed_files[file_index].statements.iter().any(|statement| {
        let ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
            module_specifier,
            ..
        }) = statement
        else {
            return false;
        };

        resolve_relative_module(
            &parsed_files[file_index].file_name,
            module_specifier,
            parsed_files,
        )
        .is_none()
    })
}

fn emit_unsupported_module_syntax_diagnostic(
    ctx: &mut CheckerContext,
    import: &ParsedImportDeclaration,
) {
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::Custom("typescript-rust::unsupported-module-syntax"),
        "Unsupported module syntax.".to_string(),
        ctx.file_name.clone(),
    );

    if let Some(span) = import.span.or(import.module_specifier_span) {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn emit_missing_export_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    export_name: &str,
    name_span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::TypeScript(2305),
        format!("Module '{module_specifier}' has no exported member '{export_name}'."),
        ctx.file_name.clone(),
    );

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn push_unresolved_export_diagnostic(
    ctx: &mut CheckerContext,
    local_name: &str,
    name_span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::ts2304(local_name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn attach_type_resolution_scope(
    declaration: TypeDeclarationInfo,
    resolution_scope: Rc<TypeDeclarationTable>,
) -> TypeDeclarationInfo {
    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            alias.resolution_scope = Some(resolution_scope);
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            interface.resolution_scope = Some(resolution_scope);
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeSpecifierKind {
    ExplicitTs,
    Extensionless,
    Unsupported,
}

fn relative_specifier_kind(specifier: &str) -> RelativeSpecifierKind {
    let last_segment = specifier.rsplit('/').next().unwrap_or(specifier);

    if last_segment.ends_with(".d.ts") {
        return RelativeSpecifierKind::Unsupported;
    }

    if last_segment.ends_with(".ts") {
        return RelativeSpecifierKind::ExplicitTs;
    }

    if last_segment.contains('.') {
        return RelativeSpecifierKind::Unsupported;
    }

    RelativeSpecifierKind::Extensionless
}

fn module_directory(file_name: &str) -> String {
    let normalized = normalize_module_path(file_name);
    normalized
        .rsplit_once('/')
        .map(|(directory, _)| directory.to_string())
        .unwrap_or_default()
}

fn normalize_module_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    let is_absolute = path.starts_with('/');
    let mut segments = Vec::new();

    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".." {
            if let Some(last) = segments.last() {
                if last != ".." {
                    segments.pop();
                    continue;
                }
            }

            if !is_absolute {
                segments.push(segment.to_string());
            }

            continue;
        }

        segments.push(segment.to_string());
    }

    let mut normalized = String::new();
    if is_absolute {
        normalized.push('/');
    }

    normalized.push_str(&segments.join("/"));

    if normalized.is_empty() {
        if is_absolute {
            "/".to_string()
        } else {
            ".".to_string()
        }
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(files: &[(&str, &str)]) -> Vec<ParsedProgramFile> {
        files
            .iter()
            .map(|(file_name, source_text)| {
                let parsed = typescript_rust_syntax::parse_source(source_text, file_name);
                ParsedProgramFile {
                    file_name: parsed.file_name,
                    source_text: (*source_text).to_string(),
                    statements: parsed.statements,
                    parser_errors: parsed.parser_errors,
                    is_module: parsed.is_module,
                }
            })
            .collect()
    }

    #[test]
    fn module_resolver_relative_same_dir_extensionless() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_relative_same_dir_with_ts_extension() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user.ts", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_relative_parent_dir() {
        let files = program(&[("src/index.ts", "export {}"), ("user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "user.ts");
    }

    #[test]
    fn module_resolver_relative_dot_segments() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/pages/index.ts", ".././user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_relative_windows_separators() {
        let files = program(&[("src\\index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src\\index.ts", ".\\user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_non_relative_unsupported_loaded_files_only() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "pkg", &files).is_none());
    }

    #[test]
    fn module_resolver_missing_file() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "./missing", &files).is_none());
    }

    #[test]
    fn module_resolver_index_file_optional_policy() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_resolves_script_file_target_for_side_effect() {
        let files = program(&[
            ("src/index.ts", "import \"./setup\";"),
            ("src/setup.ts", "let initialized = true;"),
        ]);
        let resolved = resolve_relative_module("src/index.ts", "./setup", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/setup.ts");
    }

    #[test]
    fn module_resolver_resolves_module_file_target_for_side_effect() {
        let files = program(&[
            ("src/index.ts", "import \"./setup\";"),
            ("src/setup.ts", "export {};"),
        ]);
        let resolved = resolve_relative_module("src/index.ts", "./setup", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/setup.ts");
    }

    #[test]
    fn module_resolver_named_import_from_script_file_is_resolved_but_not_exported() {
        let files = program(&[
            ("src/index.ts", "import { value } from \"./setup\";"),
            ("src/setup.ts", "let value = 1;"),
        ]);
        let resolved = resolve_relative_module("src/index.ts", "./setup", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/setup.ts");
    }

    #[test]
    fn module_resolver_marks_unresolved_star_exports() {
        let files = program(&[("src/index.ts", "export * from \"./missing\";")]);
        let mut ctx = CheckerContext::new("src/index.ts".to_string(), Default::default());
        let local_tables = files
            .iter()
            .map(|file| {
                let local_types = TypeDeclarationTable::new();
                let local_symbols = SymbolTable::new();
                Some(build_module_export_table(
                    file,
                    &local_types,
                    &local_symbols,
                    &mut ctx,
                ))
            })
            .collect::<Vec<_>>();
        let resolved = resolve_module_export_tables(&files, &local_tables, &mut ctx);

        assert!(
            resolved[0]
                .as_ref()
                .map(|table| table.has_unresolved_star_export)
                .unwrap_or(false)
        );
    }

    #[test]
    fn module_resolver_extensionless_ts() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_explicit_ts_exact() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user.ts", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_parent_directory() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_dot_segments() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", ".././user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_windows_importer_path() {
        let files = program(&[("src\\index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src\\index.ts", "./user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_windows_specifier() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", ".\\user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_index_file() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user/index.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user/index.ts");
    }

    #[test]
    fn module_resolver_non_relative_unsupported() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "pkg", &files).is_none());
    }

    #[test]
    fn module_resolver_missing_relative_file() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "./missing", &files).is_none());
    }

    #[test]
    fn module_resolver_does_not_read_disk() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "./missing", &files).is_none());
    }

    #[test]
    fn module_resolver_does_not_match_tsx_js_json_dts() {
        let files = program(&[
            ("src/index.ts", "export {}"),
            ("src/user.tsx", "export {}"),
            ("src/user.js", "export {}"),
            ("src/user.jsx", "export {}"),
            ("src/user.json", "export {}"),
            ("src/user.d.ts", "export {}"),
        ]);

        assert!(resolve_relative_module("src/index.ts", "./user.tsx", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.js", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.jsx", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.json", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.d.ts", &files).is_none());
    }
}
