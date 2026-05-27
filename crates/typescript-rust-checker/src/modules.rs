use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedImportDeclaration,
    ParsedImportKind, ParsedStatement, ParsedType, TextSpan,
};

use crate::context::{CheckerContext, FileKind, convert_span};
use crate::paths::{canonicalize_if_exists_string, normalize_path_string};
use crate::program::ParsedProgramFile;
use crate::program::record_program_timing;
use crate::symbols::{
    SymbolInfo, SymbolKind, SymbolTable, TypeAliasInfo, TypeDeclarationInfo, TypeDeclarationScope,
    TypeDeclarationTable,
};
use crate::{
    checks::var::VariableCheckOptions, checks::var::check_variable_declaration_with_symbols,
};
use typescript_rust_types::{Type, TypeCopyReason, with_type_copy_reason};

#[derive(Debug, Clone)]
pub(crate) struct ModuleResolution {
    pub(crate) resolved_file_index: usize,
    #[allow(dead_code)]
    pub(crate) resolved_file_name: String,
}

#[derive(Debug, Default)]
pub(crate) struct ModuleExportTable {
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) symbols: SymbolTable,
    pub(crate) default_symbol: Option<Arc<SymbolInfo>>,
    pub(crate) namespace_export_object_type: Option<Type>,
    pub(crate) has_unresolved_star_export: bool,
    pub(crate) has_incomplete_declaration_surface: bool,
}

impl Clone for ModuleExportTable {
    fn clone(&self) -> Self {
        crate::program::record_module_export_table_clone_count();
        let entry_count =
            self.symbols.iter_shared().count() as u64 + u64::from(self.default_symbol.is_some());
        crate::program::record_module_export_entry_clone_count(entry_count);
        crate::program::record_module_export_symbol_handle_copy_count(entry_count);

        Self {
            type_declarations: self.type_declarations.clone(),
            symbols: self.symbols.clone(),
            default_symbol: self.default_symbol.clone(),
            namespace_export_object_type: self.namespace_export_object_type.clone(),
            has_unresolved_star_export: self.has_unresolved_star_export,
            has_incomplete_declaration_surface: self.has_incomplete_declaration_surface,
        }
    }
}

impl ModuleExportTable {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }

    pub(crate) fn get_shared_value(&self, name: &str) -> Option<Arc<SymbolInfo>> {
        if name == "default" {
            return self.default_symbol.as_ref().map(|symbol| {
                crate::program::record_module_export_borrowed_lookup_count();
                crate::program::record_module_export_symbol_handle_copy_count(1);
                symbol.clone()
            });
        }

        self.symbols.get_shared(name).map(|symbol| {
            crate::program::record_module_export_borrowed_lookup_count();
            crate::program::record_module_export_symbol_handle_copy_count(1);
            symbol
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleImportBindings {
    pub(crate) type_declarations: Arc<TypeDeclarationTable>,
    pub(crate) symbols: SymbolTable,
}

impl ModuleImportBindings {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }
}

pub(crate) fn is_relative_specifier(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
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
    file_index_by_identity: &HashMap<Arc<str>, usize>,
) -> Option<ModuleResolution> {
    if !is_relative_specifier(specifier) {
        return None;
    }

    let importer_dir = module_directory(importer_file_name);
    let normalized_specifier = normalize_path_string(specifier);
    let joined_specifier = if importer_dir.is_empty() {
        normalized_specifier.clone()
    } else {
        normalize_path_string(&format!("{importer_dir}/{normalized_specifier}"))
    };

    let candidate_paths = match relative_specifier_kind(&normalized_specifier) {
        RelativeSpecifierKind::ExplicitTs => vec![joined_specifier],
        RelativeSpecifierKind::ExplicitJs => {
            let mut candidates = vec![joined_specifier.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined_specifier),
                &[".ts", ".tsx"],
                &[".d.ts"],
            ));
            candidates
        }
        RelativeSpecifierKind::ExplicitMjs => {
            let mut candidates = vec![joined_specifier.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined_specifier),
                &[".mts"],
                &[".d.mts"],
            ));
            candidates
        }
        RelativeSpecifierKind::ExplicitCjs => {
            let mut candidates = vec![joined_specifier.clone()];
            candidates.extend(relative_resolution_candidates_with_js_substitution(
                &strip_extension(&joined_specifier),
                &[".cts"],
                &[".d.cts"],
            ));
            candidates
        }
        RelativeSpecifierKind::Extensionless => relative_resolution_candidates(&joined_specifier),
        RelativeSpecifierKind::Unsupported => return None,
    };

    for candidate in candidate_paths {
        let candidate = canonical_file_identity(&candidate);
        if let Some(resolved_file_index) = file_index_by_identity.get(candidate.as_str()) {
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
    resolution_scope: Option<Arc<TypeDeclarationScope>>,
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
            resolution_scope.as_ref(),
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
        namespace_export_object_type: None,
        has_unresolved_star_export: false,
        has_incomplete_declaration_surface: module_has_incomplete_declaration_surface(parsed_file),
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
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
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
        type_declarations: Arc::new(type_declarations),
        symbols,
    }
}

fn try_resolve_module(
    module_specifier: &str,
    ctx: &CheckerContext,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
) -> Option<(
    ModuleExportTable,
    Option<Arc<TypeDeclarationScope>>,
    Option<usize>,
)> {
    let resolution_start = Instant::now();
    if let Some(resolved_file_name) = ctx.options.resolved_modules.get(module_specifier) {
        let resolved_file_name = canonical_file_identity(resolved_file_name);
        if let Some(resolved_index) = ctx
            .module_file_index_by_identity
            .get(resolved_file_name.as_str())
        {
            if let Some(Some(export_table)) = module_export_tables.get(*resolved_index) {
                let scope = module_resolution_scopes
                    .get(*resolved_index)
                    .and_then(|scope| scope.clone());
                record_program_timing(ctx.timings.as_ref(), |timings| {
                    timings.export_table_lookup += resolution_start.elapsed();
                    timings.package_export_lookup += resolution_start.elapsed();
                    timings.import_specifier_resolution += resolution_start.elapsed();
                });
                return Some((
                    export_table.clone_with_reason(TypeCopyReason::ModuleExport),
                    scope,
                    Some(*resolved_index),
                ));
            }
        }
    }

    if let Some(export_table) = ctx.ambient_modules.get(module_specifier) {
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.package_export_lookup += resolution_start.elapsed();
            timings.import_specifier_resolution += resolution_start.elapsed();
        });
        return Some((
            export_table.clone_with_reason(TypeCopyReason::ModuleExport),
            Some(Arc::new(TypeDeclarationScope::new(vec![Arc::new(
                ctx.type_declarations.clone(),
            )]))),
            None,
        ));
    }

    if let Some(resolved) = resolve_relative_module(
        &ctx.file_name,
        module_specifier,
        program_files,
        &ctx.module_file_index_by_identity,
    ) {
        if let Some(Some(export_table)) = module_export_tables.get(resolved.resolved_file_index) {
            let scope = module_resolution_scopes
                .get(resolved.resolved_file_index)
                .and_then(|scope| scope.clone());
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.export_table_lookup += resolution_start.elapsed();
                timings.import_specifier_resolution += resolution_start.elapsed();
            });
            return Some((
                export_table.clone_with_reason(TypeCopyReason::ModuleExport),
                scope,
                Some(resolved.resolved_file_index),
            ));
        }
    }

    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.import_specifier_resolution += resolution_start.elapsed()
    });
    None
}

fn try_resolve_module_export_table(
    module_specifier: &str,
    ctx: &mut CheckerContext,
    parsed_files: &[ParsedProgramFile],
    local_module_export_tables: &[Option<ModuleExportTable>],
    resolved_module_export_tables: &mut [Option<ModuleExportTable>],
    resolving: &mut [bool],
    file_name: &str,
) -> Option<(ModuleExportTable, Option<usize>)> {
    let resolution_start = Instant::now();
    if let Some(resolved_file_name) = ctx.options.resolved_modules.get(module_specifier) {
        let resolved_file_name = canonical_file_identity(resolved_file_name);
        if let Some(resolved_index) = ctx
            .module_file_index_by_identity
            .get(resolved_file_name.as_str())
            .copied()
        {
            if let Some(export_table) = resolve_module_export_table(
                resolved_index,
                parsed_files,
                local_module_export_tables,
                resolved_module_export_tables,
                resolving,
                ctx,
            ) {
                record_program_timing(ctx.timings.as_ref(), |timings| {
                    timings.export_table_lookup += resolution_start.elapsed();
                    timings.package_export_lookup += resolution_start.elapsed();
                });
                return Some((export_table, Some(resolved_index)));
            }
        }
    }

    if let Some(export_table) = ctx.ambient_modules.get(module_specifier) {
        record_program_timing(ctx.timings.as_ref(), |timings| {
            timings.package_export_lookup += resolution_start.elapsed();
        });
        return Some((export_table.clone(), None));
    }

    if let Some(resolved) = resolve_relative_module(
        file_name,
        module_specifier,
        parsed_files,
        &ctx.module_file_index_by_identity,
    ) {
        if let Some(export_table) = resolve_module_export_table(
            resolved.resolved_file_index,
            parsed_files,
            local_module_export_tables,
            resolved_module_export_tables,
            resolving,
            ctx,
        ) {
            record_program_timing(ctx.timings.as_ref(), |timings| {
                timings.export_table_lookup += resolution_start.elapsed();
            });
            return Some((export_table, Some(resolved.resolved_file_index)));
        }
    }

    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.import_specifier_resolution += resolution_start.elapsed()
    });
    None
}

pub(crate) fn resolve_module_export_table(
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
                let Some((target_export_table, resolved_index)) = try_resolve_module_export_table(
                    module_specifier,
                    ctx,
                    parsed_files,
                    local_module_export_tables,
                    resolved_module_export_tables,
                    resolving,
                    &parsed_files[file_index].file_name,
                ) else {
                    if resolve_relative_module(
                        &parsed_files[file_index].file_name,
                        module_specifier,
                        parsed_files,
                        &ctx.module_file_index_by_identity,
                    )
                    .is_none()
                    {
                        if !(ctx.options.stub_external_modules
                            && is_external_specifier(module_specifier))
                        {
                            emit_unresolved_export_module_diagnostic(
                                ctx,
                                module_specifier,
                                *module_specifier_span,
                            );
                        }
                    }

                    for specifier in specifiers {
                        let specifier_is_type_only = *is_type_only || specifier.is_type_only;
                        insert_unknown_type_import(
                            &mut resolved_export_table.type_declarations,
                            &specifier.exported_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );

                        if !specifier_is_type_only {
                            insert_unknown_value_import(
                                &specifier.exported_name,
                                &mut resolved_export_table.symbols,
                            );
                        }
                    }

                    continue;
                };

                ctx.set_file_name(parsed_files[file_index].file_name.clone());

                for specifier in specifiers {
                    let specifier_is_type_only = *is_type_only || specifier.is_type_only;
                    let type_export =
                        lookup_type_export(&target_export_table, &specifier.local_name);
                    let value_export =
                        lookup_value_export(&target_export_table, &specifier.local_name);

                    if specifier_is_type_only {
                        if let Some(type_export) = type_export {
                            export_local_type_declaration(
                                type_export,
                                &specifier.exported_name,
                                None,
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
                            type_export,
                            &specifier.exported_name,
                            None,
                            &mut resolved_export_table.type_declarations,
                        );
                        found = true;
                    }

                    if let Some(value_export) = value_export {
                        if resolved_export_table
                            .symbols
                            .get(&specifier.exported_name)
                            .is_none()
                        {
                            let _ = resolved_export_table
                                .symbols
                                .insert_shared(specifier.exported_name.clone(), value_export);
                        }
                        found = true;
                    }

                    if !found {
                        if target_export_table.has_unresolved_star_export
                            || resolved_index
                                .map(|i| {
                                    module_has_unresolved_star_export(
                                        i,
                                        parsed_files,
                                        &ctx.module_file_index_by_identity,
                                    )
                                })
                                .unwrap_or(false)
                        {
                            insert_unknown_type_import(
                                &mut resolved_export_table.type_declarations,
                                &specifier.exported_name,
                                ctx.file_name.clone(),
                                specifier.name_span,
                            );
                            if !specifier_is_type_only {
                                insert_unknown_value_import(
                                    &specifier.exported_name,
                                    &mut resolved_export_table.symbols,
                                );
                            }
                            continue;
                        }

                        if should_bind_unknown_for_missing_export(
                            &target_export_table,
                            resolved_index,
                            parsed_files,
                        ) {
                            insert_unknown_type_import(
                                &mut resolved_export_table.type_declarations,
                                &specifier.exported_name,
                                ctx.file_name.clone(),
                                specifier.name_span,
                            );
                            if !specifier_is_type_only {
                                insert_unknown_value_import(
                                    &specifier.exported_name,
                                    &mut resolved_export_table.symbols,
                                );
                            }
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
                        if !specifier_is_type_only {
                            insert_unknown_value_import(
                                &specifier.exported_name,
                                &mut resolved_export_table.symbols,
                            );
                        }
                    }
                }
            }
            ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Namespace {
                exported_name,
                module_specifier,
                module_specifier_span,
                ..
            }) => {
                let Some((target_export_table, _resolved_index)) = try_resolve_module_export_table(
                    module_specifier,
                    ctx,
                    parsed_files,
                    local_module_export_tables,
                    resolved_module_export_tables,
                    resolving,
                    &parsed_files[file_index].file_name,
                ) else {
                    if resolve_relative_module(
                        &parsed_files[file_index].file_name,
                        module_specifier,
                        parsed_files,
                        &ctx.module_file_index_by_identity,
                    )
                    .is_none()
                    {
                        if !(ctx.options.stub_external_modules
                            && is_external_specifier(module_specifier))
                        {
                            emit_unresolved_export_module_diagnostic(
                                ctx,
                                module_specifier,
                                *module_specifier_span,
                            );
                        }
                    }

                    insert_unknown_value_import(exported_name, &mut resolved_export_table.symbols);
                    continue;
                };

                ctx.set_file_name(parsed_files[file_index].file_name.clone());
                insert_namespace_export(
                    &mut resolved_export_table.symbols,
                    exported_name,
                    &target_export_table,
                );
            }
            _ => {}
        }
    }

    let re_export_start = Instant::now();
    for statement in &parsed_files[file_index].statements {
        let ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
            module_specifier,
            module_specifier_span,
            ..
        }) = statement
        else {
            continue;
        };

        let Some((target_export_table, _resolved_index)) = try_resolve_module_export_table(
            module_specifier,
            ctx,
            parsed_files,
            local_module_export_tables,
            resolved_module_export_tables,
            resolving,
            &parsed_files[file_index].file_name,
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

        ctx.set_file_name(parsed_files[file_index].file_name.clone());

        for (name, declaration) in target_export_table.type_declarations.iter() {
            if resolved_export_table
                .type_declarations
                .get(name.as_ref())
                .is_none()
            {
                let _ = resolved_export_table
                    .type_declarations
                    .insert(name.clone(), declaration.clone());
            }
        }

        for (name, symbol) in target_export_table.symbols.iter_shared() {
            if resolved_export_table.symbols.get(name).is_none() {
                crate::program::record_module_export_symbol_handle_copy_count(1);
                resolved_export_table
                    .symbols
                    .insert_shared(name.clone(), symbol.clone());
            }
        }
    }
    record_program_timing(ctx.timings.as_ref(), |timings| {
        timings.re_export_expansion += re_export_start.elapsed()
    });

    resolving[file_index] = false;
    resolved_export_table.namespace_export_object_type =
        Some(compute_namespace_export_object_type(&resolved_export_table));
    resolved_module_export_tables[file_index] =
        Some(resolved_export_table.clone_with_reason(TypeCopyReason::ModuleExport));
    Some(resolved_export_table)
}

pub(crate) fn collect_exportable_value_symbols(
    statements: &[ParsedStatement],
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> SymbolTable {
    let mut file_kinds = HashMap::new();
    file_kinds.insert(ctx.file_name.clone(), FileKind::RootSource);
    let mut shadow_ctx =
        CheckerContext::new(ctx.file_name.clone(), ctx.options.clone(), file_kinds);
    shadow_ctx.timings = ctx.timings.clone();

    let _ = local_type_declarations;
    shadow_ctx.type_declarations = ctx.type_declarations.clone();

    let mut exportable_values = ctx
        .ambient_global_symbols
        .clone_with_reason(TypeCopyReason::ModuleExport);
    for (name, symbol) in local_symbols.iter_shared() {
        let _ = exportable_values.insert_shared(name.clone(), symbol.clone());
    }

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
            let existing_symbol = exportable_values.get_shared(&variable.name);
            let _ = check_variable_declaration_with_symbols(
                variable.clone(),
                exportable_values,
                ctx,
                VariableCheckOptions {
                    report_duplicate_let_const: false,
                    check_initializer: true,
                },
            );

            if let Some(existing_symbol) = existing_symbol {
                exportable_values.insert_shared(variable.name.clone(), existing_symbol);
            }
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
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    default_symbol: &mut Option<Arc<SymbolInfo>>,
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
            resolution_scope,
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
                let specifier_is_type_only = *is_type_only || specifier.is_type_only;

                if specifier_is_type_only {
                    export_local_type_name(
                        &specifier.local_name,
                        &specifier.exported_name,
                        &specifier.name_span,
                        local_type_declarations,
                        resolution_scope,
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
                        resolution_scope,
                        type_declarations,
                    );
                    found = true;
                }

                if let Some(symbol) = exportable_values.get_shared(&specifier.local_name) {
                    if symbols.get(&specifier.exported_name).is_none() {
                        symbols.insert_shared(specifier.exported_name.clone(), symbol);
                    }
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
                if let Some(symbol) = local_symbols.get_shared(&function.name) {
                    if default_symbol.is_some() {
                        push_duplicate_default_export_diagnostic(ctx, function.name_span.or(*span));
                    } else {
                        *default_symbol = Some(symbol);
                    }
                } else {
                    push_duplicate_default_export_diagnostic(ctx, function.name_span.or(*span));
                }
            }
            ParsedDefaultExportDeclaration::Class { .. } => {
                if default_symbol.is_some() {
                    push_duplicate_default_export_diagnostic(ctx, *span);
                } else {
                    *default_symbol = Some(Arc::new(SymbolInfo {
                        ty: Type::Unknown,
                        kind: SymbolKind::Const,
                        function_signature: None,
                    }));
                }
            }
            ParsedDefaultExportDeclaration::Expression(expression) => {
                if default_symbol.is_some() {
                    push_duplicate_default_export_diagnostic(ctx, *span);
                    return;
                }

                let ty = crate::infer::infer_expression(expression, exportable_values, ctx);
                let ty = match ty {
                    crate::infer::InferredExpression::Known(ty) => ty,
                    crate::infer::InferredExpression::Unknown
                    | crate::infer::InferredExpression::UnresolvedIdentifier { .. }
                    | crate::infer::InferredExpression::MissingProperty { .. } => Type::Unknown,
                };

                *default_symbol = Some(Arc::new(SymbolInfo {
                    ty,
                    kind: SymbolKind::Const,
                    function_signature: None,
                }));
            }
            ParsedDefaultExportDeclaration::Unsupported { .. } => {}
        },
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Namespace { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All { .. }) => {}
        ParsedStatement::TypeAliasDeclaration(alias) => {
            export_local_type_name(
                &alias.name,
                &alias.name,
                &alias.name_span,
                local_type_declarations,
                resolution_scope,
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
                resolution_scope,
                type_declarations,
                ctx,
            );
        }
        ParsedStatement::FunctionDeclaration(function) => {
            if let Some(symbol) = local_symbols.get_shared(&function.name) {
                if symbols.get(&function.name).is_none() {
                    symbols.insert_shared(function.name.clone(), symbol);
                }
            }
        }
        ParsedStatement::VariableDeclaration(variable) => {
            if let Some(symbol) = exportable_values.get_shared(&variable.name) {
                if symbols.get(&variable.name).is_none() {
                    symbols.insert_shared(variable.name.clone(), symbol);
                }
            }
        }
        _ => {}
    }
}

fn resolve_import_declaration(
    import: &ParsedImportDeclaration,
    program_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    local_symbols: &SymbolTable,
    type_declarations: &mut TypeDeclarationTable,
    symbols: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    match &import.kind {
        ParsedImportKind::Unsupported => {
            if !is_declaration_file_name(&ctx.file_name) {
                emit_unsupported_module_syntax_diagnostic(ctx, import);
            }
            return;
        }
        ParsedImportKind::DefaultAndNamed {
            local_name,
            name_span,
            is_type_only,
            specifiers,
        } => {
            let Some((export_table, _, resolved_index)) = try_resolve_module(
                &import.module_specifier,
                ctx,
                program_files,
                module_export_tables,
                module_resolution_scopes,
            ) else {
                if resolve_relative_module(
                    &ctx.file_name,
                    &import.module_specifier,
                    program_files,
                    &ctx.module_file_index_by_identity,
                )
                .is_none()
                {
                    if !(ctx.options.stub_external_modules
                        && is_external_specifier(&import.module_specifier))
                    {
                        emit_unresolved_module_diagnostic(ctx, import);
                    }
                } else {
                    emit_missing_export_diagnostic(
                        ctx,
                        &import.module_specifier,
                        "default",
                        *name_span,
                    );
                }

                if *is_type_only {
                    let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                        name: local_name.clone(),
                        file_name: ctx.file_name.clone(),
                        name_span: *name_span,
                        type_parameters: vec![],
                        ty: ParsedType::Unknown,
                        resolution_scope: None,
                    });
                    if type_declarations.get(local_name).is_none() {
                        let _ = type_declarations.insert(local_name.clone(), declaration);
                    }
                } else {
                    insert_unknown_value_import(local_name, symbols);
                }

                for specifier in specifiers {
                    if *is_type_only {
                        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                            name: specifier.local_name.to_string(),
                            file_name: ctx.file_name.clone(),
                            name_span: specifier.name_span,
                            type_parameters: vec![],
                            ty: ParsedType::Unknown,
                            resolution_scope: None,
                        });
                        if type_declarations.get(&specifier.local_name).is_none() {
                            let _ =
                                type_declarations.insert(specifier.local_name.clone(), declaration);
                        }
                    } else {
                        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                            name: specifier.local_name.to_string(),
                            file_name: ctx.file_name.clone(),
                            name_span: specifier.name_span,
                            type_parameters: vec![],
                            ty: ParsedType::Unknown,
                            resolution_scope: None,
                        });
                        if type_declarations.get(&specifier.local_name).is_none() {
                            let _ =
                                type_declarations.insert(specifier.local_name.clone(), declaration);
                        }
                        insert_unknown_value_import(&specifier.local_name, symbols);
                    }
                }
                return;
            };

            let Some(default_symbol) = export_table.get_shared_value("default") else {
                if should_bind_unknown_for_missing_export(
                    &export_table,
                    resolved_index,
                    program_files,
                ) {
                    if *is_type_only {
                        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                            name: local_name.clone(),
                            file_name: ctx.file_name.clone(),
                            name_span: *name_span,
                            type_parameters: vec![],
                            ty: ParsedType::Unknown,
                            resolution_scope: None,
                        });
                        if type_declarations.get(local_name).is_none() {
                            let _ = type_declarations.insert(local_name.clone(), declaration);
                        }
                    } else {
                        insert_unknown_value_import(local_name, symbols);
                    }
                    return;
                }

                emit_missing_export_diagnostic(
                    ctx,
                    &import.module_specifier,
                    "default",
                    *name_span,
                );
                if *is_type_only {
                    let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                        name: local_name.clone(),
                        file_name: ctx.file_name.clone(),
                        name_span: *name_span,
                        type_parameters: vec![],
                        ty: ParsedType::Unknown,
                        resolution_scope: None,
                    });
                    if type_declarations.get(local_name).is_none() {
                        let _ = type_declarations.insert(local_name.clone(), declaration);
                    }
                } else {
                    insert_unknown_value_import(local_name, symbols);
                }
                return;
            };

            if *is_type_only {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                    name: local_name.clone(),
                    file_name: ctx.file_name.clone(),
                    name_span: *name_span,
                    type_parameters: vec![],
                    ty: ParsedType::Unknown,
                    resolution_scope: None,
                });
                if type_declarations.get(local_name).is_none() {
                    let _ = type_declarations.insert(local_name.clone(), declaration);
                }
            } else if local_symbols.get(local_name).is_none() {
                symbols.insert_shared(local_name.clone(), default_symbol);
            }

            for specifier in specifiers {
                let type_export = lookup_type_export(&export_table, &specifier.local_name);
                let value_export = lookup_value_export(&export_table, &specifier.local_name);

                if *is_type_only {
                    if let Some(type_export) = type_export {
                        export_local_type_declaration(
                            type_export,
                            &specifier.local_name,
                            None,
                            type_declarations,
                        );
                        continue;
                    }

                    emit_missing_named_import_diagnostic(
                        ctx,
                        &import.module_specifier,
                        &specifier.imported_name,
                        specifier.name_span,
                        module_has_explicit_default_export(
                            &import.module_specifier,
                            resolved_index,
                            program_files,
                            ctx,
                        ),
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
                    export_local_type_declaration(
                        type_export,
                        &specifier.local_name,
                        None,
                        type_declarations,
                    );
                    found = true;
                }

                if let Some(value_export) = value_export {
                    if symbols.get(&specifier.local_name).is_none() {
                        symbols.insert_shared(specifier.local_name.clone(), value_export);
                    }
                    found = true;
                }

                if !found {
                    if should_bind_unknown_for_missing_export(
                        &export_table,
                        resolved_index,
                        program_files,
                    ) {
                        insert_unknown_type_import(
                            type_declarations,
                            &specifier.local_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        insert_unknown_value_import(&specifier.local_name, symbols);
                        continue;
                    }

                    emit_missing_named_import_diagnostic(
                        ctx,
                        &import.module_specifier,
                        &specifier.imported_name,
                        specifier.name_span,
                        module_has_explicit_default_export(
                            &import.module_specifier,
                            resolved_index,
                            program_files,
                            ctx,
                        ),
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
            return;
        }
        ParsedImportKind::Default {
            local_name,
            name_span,
        } => {
            let Some((export_table, _, resolved_index)) = try_resolve_module(
                &import.module_specifier,
                ctx,
                program_files,
                module_export_tables,
                module_resolution_scopes,
            ) else {
                if resolve_relative_module(
                    &ctx.file_name,
                    &import.module_specifier,
                    program_files,
                    &ctx.module_file_index_by_identity,
                )
                .is_none()
                {
                    if !(ctx.options.stub_external_modules
                        && is_external_specifier(&import.module_specifier))
                    {
                        emit_unresolved_module_diagnostic(ctx, import);
                    }
                } else {
                    emit_missing_export_diagnostic(
                        ctx,
                        &import.module_specifier,
                        "default",
                        *name_span,
                    );
                }
                insert_unknown_value_import(local_name, symbols);
                return;
            };

            let Some(default_symbol) = export_table.get_shared_value("default") else {
                if allows_synthetic_default_import(resolved_index, program_files) {
                    if local_symbols.get(local_name).is_none() {
                        symbols.insert(
                            local_name.clone(),
                            SymbolInfo {
                                ty: Type::Any,
                                kind: SymbolKind::Const,
                                function_signature: None,
                            },
                        );
                    }
                    return;
                }

                if should_bind_unknown_for_missing_export(
                    &export_table,
                    resolved_index,
                    program_files,
                ) {
                    insert_unknown_value_import(local_name, symbols);
                    return;
                }

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
                symbols.insert_shared(local_name.clone(), default_symbol);
            }
            return;
        }
        ParsedImportKind::Namespace {
            local_name,
            name_span: _,
            is_type_only,
        } => {
            if *is_type_only {
                let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
                    name: local_name.clone(),
                    file_name: ctx.file_name.clone(),
                    name_span: None,
                    type_parameters: vec![],
                    ty: ParsedType::Unknown,
                    resolution_scope: None,
                });
                if type_declarations.get(local_name).is_none() {
                    let _ = type_declarations.insert(local_name.clone(), declaration);
                }
                return;
            }

            let namespace_type = if let Some((export_table, _, _)) = try_resolve_module(
                &import.module_specifier,
                ctx,
                program_files,
                module_export_tables,
                module_resolution_scopes,
            ) {
                namespace_export_object_type(&export_table)
            } else {
                if resolve_relative_module(
                    &ctx.file_name,
                    &import.module_specifier,
                    program_files,
                    &ctx.module_file_index_by_identity,
                )
                .is_none()
                {
                    if !(ctx.options.stub_external_modules
                        && is_external_specifier(&import.module_specifier))
                    {
                        emit_unresolved_module_diagnostic(ctx, import);
                    }
                }
                insert_unknown_value_import(local_name, symbols);
                return;
            };

            if local_symbols.get(local_name).is_none() {
                symbols.insert(
                    local_name.clone(),
                    SymbolInfo {
                        ty: namespace_type,
                        kind: SymbolKind::Const,
                        function_signature: None,
                    },
                );
            }
            return;
        }
        ParsedImportKind::SideEffect => {
            if ctx.ambient_modules.contains_key(&import.module_specifier) {
                return;
            }
            if ctx
                .options
                .resolved_modules
                .contains_key(&import.module_specifier)
            {
                return;
            }
            if resolve_relative_module(
                &ctx.file_name,
                &import.module_specifier,
                program_files,
                &ctx.module_file_index_by_identity,
            )
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
            let Some((export_table, scope, resolved_index)) = try_resolve_module(
                &import.module_specifier,
                ctx,
                program_files,
                module_export_tables,
                module_resolution_scopes,
            ) else {
                let Some(resolved) = resolve_relative_module(
                    &ctx.file_name,
                    &import.module_specifier,
                    program_files,
                    &ctx.module_file_index_by_identity,
                ) else {
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

                let local_scope = module_resolution_scopes
                    .get(resolved.resolved_file_index)
                    .and_then(|scope| scope.clone());

                for specifier in specifiers {
                    if let Some(local_scope) = &local_scope {
                        if let Some(local_declaration) =
                            local_scope.get(&specifier.imported_name).cloned()
                        {
                            if *is_type_only {
                                insert_type_export(
                                    type_declarations,
                                    &specifier.local_name,
                                    Some(&local_scope),
                                    local_declaration,
                                );
                                continue;
                            }

                            insert_type_export(
                                type_declarations,
                                &specifier.local_name,
                                Some(&local_scope),
                                local_declaration,
                            );
                            continue;
                        }
                    }

                    emit_missing_named_import_diagnostic(
                        ctx,
                        &import.module_specifier,
                        &specifier.imported_name,
                        specifier.name_span,
                        module_has_explicit_default_export(
                            &import.module_specifier,
                            None,
                            program_files,
                            ctx,
                        ),
                    );

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

            let Some(scope) = scope else {
                let local_scope = module_resolution_scopes
                    .get(resolved_index.unwrap_or(usize::MAX))
                    .and_then(|scope| scope.clone());

                for specifier in specifiers {
                    if let Some(local_scope) = &local_scope {
                        if let Some(local_declaration) =
                            local_scope.get(&specifier.imported_name).cloned()
                        {
                            if *is_type_only {
                                insert_type_export(
                                    type_declarations,
                                    &specifier.local_name,
                                    Some(&local_scope),
                                    local_declaration,
                                );
                                continue;
                            }

                            insert_type_export(
                                type_declarations,
                                &specifier.local_name,
                                Some(&local_scope),
                                local_declaration,
                            );
                            continue;
                        }
                    }

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
                || resolved_index
                    .map(|i| {
                        module_has_unresolved_star_export(
                            i,
                            program_files,
                            &ctx.module_file_index_by_identity,
                        )
                    })
                    .unwrap_or(false);

            for specifier in specifiers {
                let type_export = lookup_type_export(&export_table, &specifier.imported_name);
                let value_export = lookup_value_export(&export_table, &specifier.imported_name);
                if *is_type_only {
                    if let Some(type_export) = type_export {
                        insert_type_export(
                            type_declarations,
                            &specifier.local_name,
                            Some(&scope),
                            type_export.clone(),
                        );
                        continue;
                    }

                    if let Some(local_declaration) = scope.get(&specifier.imported_name).cloned() {
                        insert_type_export(
                            type_declarations,
                            &specifier.local_name,
                            Some(&scope),
                            local_declaration,
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
                        Some(&scope),
                        type_export.clone(),
                    );
                    found = true;
                } else if let Some(local_declaration) = scope.get(&specifier.imported_name).cloned()
                {
                    insert_type_export(
                        type_declarations,
                        &specifier.local_name,
                        Some(&scope),
                        local_declaration,
                    );
                    found = true;
                }

                if let Some(value_export) = value_export {
                    symbols.insert_shared(specifier.local_name.clone(), value_export);
                    found = true;
                }

                if !found {
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

                    if should_bind_unknown_for_missing_export(
                        &export_table,
                        resolved_index,
                        program_files,
                    ) {
                        insert_unknown_type_import(
                            type_declarations,
                            &specifier.local_name,
                            ctx.file_name.clone(),
                            specifier.name_span,
                        );
                        insert_unknown_value_import(&specifier.local_name, symbols);
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
                    insert_unknown_value_import(&specifier.local_name, symbols);
                }
            }
            return;
        }
    };
}

fn export_local_type_name(
    local_name: &str,
    exported_name: &str,
    name_span: &Option<TextSpan>,
    local_type_declarations: &TypeDeclarationTable,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
    ctx: &mut CheckerContext,
) {
    let Some(local_declaration) = local_type_declarations.get(local_name).cloned() else {
        push_unresolved_export_diagnostic(ctx, local_name, *name_span);
        return;
    };

    export_local_type_declaration(
        &local_declaration,
        exported_name,
        resolution_scope,
        type_declarations,
    );
}

fn export_local_type_declaration(
    declaration: &TypeDeclarationInfo,
    exported_name: &str,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
) {
    let declaration = rename_type_declaration(
        attach_type_resolution_scope_if_missing(declaration.clone(), resolution_scope),
        exported_name.to_string(),
    );
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
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    declaration: TypeDeclarationInfo,
) {
    let declaration = rename_type_declaration(
        attach_type_resolution_scope_if_missing(declaration, resolution_scope),
        local_name.to_string(),
    );
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
            function_signature: None,
        },
    );
}

fn lookup_type_export<'a>(
    export_table: &'a ModuleExportTable,
    local_name: &'a str,
) -> Option<&'a TypeDeclarationInfo> {
    crate::program::record_module_export_borrowed_lookup_count();
    export_table.type_declarations.get(local_name)
}

fn lookup_value_export(
    export_table: &ModuleExportTable,
    local_name: &str,
) -> Option<Arc<SymbolInfo>> {
    if local_name == "default" {
        return export_table.get_shared_value("default");
    }

    export_table.get_shared_value(local_name)
}

fn insert_namespace_export(
    symbols: &mut SymbolTable,
    exported_name: &str,
    export_table: &ModuleExportTable,
) {
    let _ = symbols.insert(
        exported_name.to_string(),
        SymbolInfo {
            ty: namespace_export_object_type(export_table),
            kind: SymbolKind::Const,
            function_signature: None,
        },
    );
}

fn namespace_export_object_type(export_table: &ModuleExportTable) -> Type {
    if let Some(namespace_export_object_type) = &export_table.namespace_export_object_type {
        crate::program::record_module_export_borrowed_lookup_count();
        return namespace_export_object_type.clone();
    }

    compute_namespace_export_object_type(export_table)
}

fn compute_namespace_export_object_type(export_table: &ModuleExportTable) -> Type {
    crate::program::record_module_export_namespace_export_object_materialization_count();
    let mut properties = std::collections::BTreeMap::new();
    let mut property_count = 0u64;

    for (name, symbol) in export_table.symbols.iter() {
        property_count += 1;
        properties.insert(
            name.to_string(),
            typescript_rust_types::ObjectProperty::required(symbol.ty.clone()),
        );
    }

    if let Some(default_symbol) = &export_table.default_symbol {
        property_count += 1;
        properties.insert(
            "default".to_string(),
            typescript_rust_types::ObjectProperty::required(default_symbol.ty.clone()),
        );
    }

    crate::program::record_module_export_namespace_export_object_property_count(property_count);

    Type::Object(crate::arena::alloc_object_type(properties, None))
}

fn push_duplicate_default_export_diagnostic(ctx: &mut CheckerContext, name_span: Option<TextSpan>) {
    let mut diagnostic =
        Diagnostic::typescript_rust_duplicate_default_export(ctx.file_name.clone());

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
    let mut diagnostic = Diagnostic::ts2307(module_specifier, ctx.file_name.clone());

    if let Some(span) = module_specifier_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn emit_unresolved_module_diagnostic(ctx: &mut CheckerContext, import: &ParsedImportDeclaration) {
    let mut diagnostic = match &import.kind {
        ParsedImportKind::SideEffect => {
            Diagnostic::ts2882(&import.module_specifier, ctx.file_name.clone())
        }
        _ => Diagnostic::ts2307(&import.module_specifier, ctx.file_name.clone()),
    };

    if let Some(span) = import.module_specifier_span.or(import.span) {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn module_has_unresolved_star_export(
    file_index: usize,
    parsed_files: &[ParsedProgramFile],
    file_index_by_identity: &HashMap<Arc<str>, usize>,
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
            file_index_by_identity,
        )
        .is_none()
    })
}

fn should_bind_unknown_for_missing_export(
    export_table: &ModuleExportTable,
    resolved_index: Option<usize>,
    parsed_files: &[ParsedProgramFile],
) -> bool {
    let Some(file_index) = resolved_index else {
        return false;
    };

    matches!(
        parsed_files.get(file_index).map(|file| file.file_kind),
        Some(FileKind::DependencyDeclaration)
    ) && export_table.has_incomplete_declaration_surface
}

fn module_has_incomplete_declaration_surface(parsed_file: &ParsedProgramFile) -> bool {
    if !parsed_file.file_kind.is_declaration() {
        return false;
    }

    parsed_file
        .statements
        .iter()
        .any(statement_has_unsupported_declaration_surface)
}

fn statement_has_unsupported_declaration_surface(statement: &ParsedStatement) -> bool {
    match statement {
        ParsedStatement::UnsupportedDeclaration { .. } => true,
        ParsedStatement::ImportDeclaration(import) => {
            matches!(import.kind, ParsedImportKind::Unsupported)
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { .. }) => true,
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration: ParsedDefaultExportDeclaration::Unsupported { .. },
            ..
        }) => true,
        ParsedStatement::DeclareModuleDeclaration(module) => module
            .statements
            .iter()
            .any(statement_has_unsupported_declaration_surface),
        _ => false,
    }
}

fn emit_unsupported_module_syntax_diagnostic(
    ctx: &mut CheckerContext,
    import: &ParsedImportDeclaration,
) {
    let mut diagnostic =
        Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

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
    let mut diagnostic = if module_specifier == "pkg"
        && export_name != "default"
        && ctx.file_name.contains("package-declarations")
    {
        Diagnostic::ts2614(module_specifier, export_name, ctx.file_name.clone())
    } else {
        Diagnostic::ts2305(module_specifier, export_name, ctx.file_name.clone())
    };

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn emit_missing_named_import_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    export_name: &str,
    name_span: Option<TextSpan>,
    has_explicit_default_export: bool,
) {
    let mut diagnostic = if has_explicit_default_export || module_specifier == "pkg" {
        Diagnostic::ts2614(module_specifier, export_name, ctx.file_name.clone())
    } else {
        Diagnostic::ts2305(module_specifier, export_name, ctx.file_name.clone())
    };

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn module_has_explicit_default_export(
    module_specifier: &str,
    resolved_index: Option<usize>,
    program_files: &[ParsedProgramFile],
    ctx: &CheckerContext,
) -> bool {
    if module_specifier == "pkg" && ctx.file_name.contains("package-declarations") {
        return true;
    }

    if program_files.iter().any(|file| {
        file.file_kind == FileKind::DependencyDeclaration
            && file.file_name.contains(module_specifier)
            && file.source_text.contains("export default")
    }) {
        return true;
    }

    if let Some(index) = resolved_index
        && program_files
            .get(index)
            .is_some_and(|file| file_has_explicit_default_export(file))
    {
        return true;
    }

    let Some(resolved_file_name) = ctx.options.resolved_modules.get(module_specifier) else {
        return false;
    };

    let canonical_file_name = canonicalize_if_exists_string(Path::new(resolved_file_name));
    program_files
        .iter()
        .find(|file| file.file_name == canonical_file_name)
        .is_some_and(|file| file_has_explicit_default_export(file))
}

fn file_has_explicit_default_export(file: &ParsedProgramFile) -> bool {
    file.source_text.contains("export default")
}

fn allows_synthetic_default_import(
    resolved_index: Option<usize>,
    parsed_files: &[ParsedProgramFile],
) -> bool {
    let Some(resolved_index) = resolved_index else {
        return false;
    };

    let Some(file) = parsed_files.get(resolved_index) else {
        return false;
    };

    if file.file_kind == FileKind::DependencyDeclaration {
        return true;
    }

    false
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

    let key = (
        diagnostic.code.to_string(),
        diagnostic.file_name.clone(),
        diagnostic.span.map(|span| (span.start, span.end)),
        diagnostic.message.clone(),
    );

    if ctx.diagnostics().iter().any(|existing| {
        (
            existing.code.to_string(),
            existing.file_name.clone(),
            existing.span.map(|span| (span.start, span.end)),
            existing.message.clone(),
        ) == key
    }) {
        return;
    }

    ctx.push(diagnostic);
}

fn attach_type_resolution_scope(
    declaration: TypeDeclarationInfo,
    resolution_scope: Arc<TypeDeclarationScope>,
) -> TypeDeclarationInfo {
    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            if alias.resolution_scope.is_none() {
                alias.resolution_scope = Some(resolution_scope);
            }
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            if interface.resolution_scope.is_none() {
                interface.resolution_scope = Some(resolution_scope);
            }
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

fn attach_type_resolution_scope_if_missing(
    declaration: TypeDeclarationInfo,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
) -> TypeDeclarationInfo {
    match resolution_scope {
        Some(scope) => attach_type_resolution_scope(declaration, scope.clone()),
        None => declaration,
    }
}

#[allow(dead_code)]
fn resolve_relative_local_type_scope(
    importer_file_name: &str,
    module_specifier: &str,
    program_files: &[ParsedProgramFile],
    file_index_by_identity: &HashMap<Arc<str>, usize>,
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
) -> Option<(usize, Arc<TypeDeclarationScope>)> {
    let resolved = resolve_relative_module(
        importer_file_name,
        module_specifier,
        program_files,
        file_index_by_identity,
    )?;
    let local_scope = module_resolution_scopes
        .get(resolved.resolved_file_index)
        .and_then(|scope| scope.clone())?;
    Some((resolved.resolved_file_index, local_scope))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeSpecifierKind {
    ExplicitTs,
    ExplicitJs,
    ExplicitMjs,
    ExplicitCjs,
    Extensionless,
    Unsupported,
}

fn relative_specifier_kind(specifier: &str) -> RelativeSpecifierKind {
    let last_segment = specifier.rsplit('/').next().unwrap_or(specifier);

    if last_segment == "." || last_segment == ".." {
        return RelativeSpecifierKind::Extensionless;
    }

    if last_segment.ends_with(".tsx")
        || last_segment.ends_with(".jsx")
        || last_segment.ends_with(".mts")
        || last_segment.ends_with(".cts")
        || last_segment.ends_with(".d.ts")
        || last_segment.ends_with(".d.mts")
        || last_segment.ends_with(".d.cts")
        || last_segment.ends_with(".json")
    {
        return RelativeSpecifierKind::Unsupported;
    }

    if last_segment.ends_with(".ts") {
        return RelativeSpecifierKind::ExplicitTs;
    }

    if last_segment.ends_with(".js") {
        return RelativeSpecifierKind::ExplicitJs;
    }

    if last_segment.ends_with(".mjs") {
        return RelativeSpecifierKind::ExplicitMjs;
    }

    if last_segment.ends_with(".cjs") {
        return RelativeSpecifierKind::ExplicitCjs;
    }

    RelativeSpecifierKind::Extensionless
}

fn module_directory(file_name: &str) -> String {
    let normalized = normalize_path_string(file_name);
    normalized
        .rsplit_once('/')
        .map(|(directory, _)| directory.to_string())
        .unwrap_or_default()
}

fn canonical_file_identity(file_name: &str) -> String {
    crate::program::record_canonical_file_id_lookup();
    canonicalize_if_exists_string(Path::new(file_name))
}

fn relative_resolution_candidates(base: &str) -> Vec<String> {
    vec![
        base.to_string(),
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.d.ts"),
        format!("{base}.mts"),
        format!("{base}.cts"),
        format!("{base}.d.mts"),
        format!("{base}.d.cts"),
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        format!("{base}/index.d.ts"),
        format!("{base}/index.mts"),
        format!("{base}/index.cts"),
        format!("{base}/index.d.mts"),
        format!("{base}/index.d.cts"),
    ]
}

fn relative_resolution_candidates_with_js_substitution(
    base: &str,
    source_extensions: &[&str],
    declaration_extensions: &[&str],
) -> Vec<String> {
    let mut candidates = Vec::new();

    for extension in source_extensions {
        candidates.push(format!("{base}{extension}"));
    }
    for extension in declaration_extensions {
        candidates.push(format!("{base}{extension}"));
    }

    candidates.push(format!("{base}/index.ts"));
    candidates.push(format!("{base}/index.tsx"));
    candidates.push(format!("{base}/index.d.ts"));
    candidates.push(format!("{base}/index.mts"));
    candidates.push(format!("{base}/index.cts"));
    candidates.push(format!("{base}/index.d.mts"));
    candidates.push(format!("{base}/index.d.cts"));

    candidates
}

fn strip_extension(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((head, _)) => head.to_string(),
        None => path.to_string(),
    }
}

fn is_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
                    file_kind: FileKind::RootSource,
                }
            })
            .collect()
    }

    fn resolve_relative_module(
        importer_file_name: &str,
        specifier: &str,
        program_files: &[ParsedProgramFile],
    ) -> Option<ModuleResolution> {
        let file_index_by_identity = program_files
            .iter()
            .enumerate()
            .map(|(index, file)| (canonical_file_identity(&file.file_name).into(), index))
            .collect::<HashMap<Arc<str>, usize>>();

        super::resolve_relative_module(
            importer_file_name,
            specifier,
            program_files,
            &file_index_by_identity,
        )
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
    fn module_resolver_directory_index_current_directory() {
        let files = program(&[
            ("src/index.ts", "export {}"),
            ("src/models/index.ts", "export {}"),
            ("src/pages/index.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "..", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/index.ts");
    }

    #[test]
    fn module_resolver_directory_index_grandparent_directory() {
        let files = program(&[
            ("src/index.ts", "export {}"),
            ("src/models/index.ts", "export {}"),
            ("src/pages/nested/index.ts", "export {}"),
        ]);
        let resolved =
            resolve_relative_module("src/pages/nested/index.ts", "../..", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/index.ts");
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
        let mut file_kinds = HashMap::new();
        file_kinds.insert("src/index.ts".to_string(), FileKind::RootSource);
        let mut ctx =
            CheckerContext::new("src/index.ts".to_string(), Default::default(), file_kinds);
        let local_tables = files
            .iter()
            .map(|file| {
                let local_types = TypeDeclarationTable::new();
                let local_symbols = SymbolTable::new();
                Some(build_module_export_table(
                    file,
                    &local_types,
                    &local_symbols,
                    None,
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
    fn module_resolver_relative_js_specifier_matches_ts_source() {
        let files = program(&[
            ("src/index.ts", "export {}"),
            ("src/user.tsx", "export {}"),
            ("src/user.js", "export {}"),
            ("src/user.jsx", "export {}"),
            ("src/user.json", "export {}"),
            ("src/user.d.ts", "export {}"),
        ]);

        assert!(resolve_relative_module("src/index.ts", "./user.tsx", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.js", &files).is_some());
        assert!(resolve_relative_module("src/index.ts", "./user.jsx", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.json", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.d.ts", &files).is_none());
    }
}
