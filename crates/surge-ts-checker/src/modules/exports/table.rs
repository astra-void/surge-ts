use super::*;

pub(crate) fn build_module_export_table(
    parsed_file: &ParsedProgramFile,
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    imported_symbols: &SymbolTable,
    resolution_scope: Option<Arc<TypeDeclarationScope>>,
    ctx: &mut CheckerContext,
) -> ModuleExportTable {
    let split_start = crate::program::binding::analyze_split_enabled()
        .then(std::time::Instant::now);
    let exportable_values = collect_exportable_value_symbols(
        &parsed_file.statements,
        local_type_declarations,
        local_symbols,
        Some(imported_symbols),
        ctx,
    );
    crate::program::binding::analyze_split_record(3, split_start);
    let split_start = crate::program::binding::analyze_split_enabled()
        .then(std::time::Instant::now);

    let mut type_declarations = TypeDeclarationTable::new();
    let mut symbols = SymbolTable::new();
    let mut default_symbol = None;
    let mut export_assignment_symbol = None;

    for statement in &parsed_file.statements {
        collect_exports_from_statement(
            statement,
            &exportable_values,
            imported_symbols,
            local_type_declarations,
            local_symbols,
            resolution_scope.as_ref(),
            &mut type_declarations,
            &mut symbols,
            &mut default_symbol,
            &mut export_assignment_symbol,
            ctx,
        );
    }

    crate::program::binding::analyze_split_record(4, split_start);
    ModuleExportTable {
        type_declarations: Arc::new(type_declarations),
        symbols,
        default_symbol,
        export_assignment_symbol,
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

pub(crate) fn try_resolve_module_export_table(
    module_specifier: &str,
    ctx: &mut CheckerContext,
    parsed_files: &[ParsedProgramFile],
    local_module_export_tables: &[Option<ModuleExportTable>],
    resolved_module_export_tables: &mut [Option<ModuleExportTable>],
    resolving: &mut [bool],
    file_name: &str,
) -> Option<(ModuleExportTable, Option<usize>)> {
    let resolution_start = Instant::now();
    if let Some(resolved_file_name) = ctx.options.resolved_module_for(file_name, module_specifier) {
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

    if let Some(export_table) =
        crate::modules::imports::ambient_module_export_table(ctx, module_specifier)
    {
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
    // `file_index` may originate from a different vector domain than the slices
    // passed here. The relative/package resolvers and `module_file_index_by_identity`
    // index the full project file list, but some callers (ambient module binding)
    // pass a narrow local vector. A resolved index from the global domain can then
    // exceed these slices, so every access is bounds-checked and an out-of-domain
    // index yields a conservative unresolved result instead of panicking.
    if let Some(Some(resolved)) = resolved_module_export_tables.get(file_index) {
        return Some(resolved.clone());
    }

    let Some(parsed_file) = parsed_files.get(file_index) else {
        return None;
    };

    let Some(local_export_table) = local_module_export_tables
        .get(file_index)
        .and_then(|slot| slot.clone())
    else {
        return None;
    };

    match resolving.get(file_index) {
        Some(true) => return Some(local_export_table),
        Some(false) => {}
        None => return None,
    }

    if let Some(slot) = resolving.get_mut(file_index) {
        *slot = true;
    }
    ctx.set_file_name(parsed_file.file_name.clone());

    let mut resolved_export_table = local_export_table;

    for statement in &parsed_file.statements {
        let ParsedStatement::ExportDeclaration(export) = statement else {
            continue;
        };
        match export.as_ref() {
            ParsedExportDeclaration::Named {
                is_type_only,
                specifiers,
                module_specifier: Some(module_specifier),
                module_specifier_span,
                ..
            } => {
                let Some((target_export_table, resolved_index)) = try_resolve_module_export_table(
                    module_specifier,
                    ctx,
                    parsed_files,
                    local_module_export_tables,
                    resolved_module_export_tables,
                    resolving,
                    &parsed_file.file_name,
                ) else {
                    if resolve_relative_module(
                        &parsed_file.file_name,
                        module_specifier,
                        parsed_files,
                        &ctx.module_file_index_by_identity,
                    )
                    .is_none()
                    {
                        record_unresolved_external_module(ctx, module_specifier);
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
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
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

                ctx.set_file_name(parsed_file.file_name.clone());

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
                                Arc::make_mut(&mut resolved_export_table.type_declarations),
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
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
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
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
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
                                Arc::make_mut(&mut resolved_export_table.type_declarations),
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
                                Arc::make_mut(&mut resolved_export_table.type_declarations),
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
                            Arc::make_mut(&mut resolved_export_table.type_declarations),
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
            ParsedExportDeclaration::Namespace {
                exported_name,
                module_specifier,
                module_specifier_span,
                ..
            } => {
                let Some((target_export_table, _resolved_index)) = try_resolve_module_export_table(
                    module_specifier,
                    ctx,
                    parsed_files,
                    local_module_export_tables,
                    resolved_module_export_tables,
                    resolving,
                    &parsed_file.file_name,
                ) else {
                    if resolve_relative_module(
                        &parsed_file.file_name,
                        module_specifier,
                        parsed_files,
                        &ctx.module_file_index_by_identity,
                    )
                    .is_none()
                    {
                        record_unresolved_external_module(ctx, module_specifier);
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

                ctx.set_file_name(parsed_file.file_name.clone());
                insert_namespace_export(
                    &mut resolved_export_table.symbols,
                    exported_name,
                    &target_export_table,
                );
                copy_namespace_member_type_exports(
                    &target_export_table,
                    exported_name,
                    &mut resolved_export_table,
                );
            }
            // `import * as z from "./m"; export { z }` re-exports the namespace
            // binding (zod's `z`). The value symbol is carried by the local
            // export pass; the namespace's TYPE side lives only in the importing
            // file's alias scope layers, so the qualified `z.<member>` keys must
            // be materialized into the export table here for the consumer-side
            // `copy_qualified_type_exports` to find.
            ParsedExportDeclaration::Named {
                specifiers,
                module_specifier: None,
                ..
            } => {
                for specifier in specifiers {
                    let Some(namespace_module_specifier) =
                        namespace_import_module_specifier(parsed_file, &specifier.local_name)
                    else {
                        continue;
                    };
                    let Some((target_export_table, _resolved_index)) =
                        try_resolve_module_export_table(
                            &namespace_module_specifier,
                            ctx,
                            parsed_files,
                            local_module_export_tables,
                            resolved_module_export_tables,
                            resolving,
                            &parsed_file.file_name,
                        )
                    else {
                        continue;
                    };
                    ctx.set_file_name(parsed_file.file_name.clone());
                    copy_namespace_member_type_exports(
                        &target_export_table,
                        &specifier.exported_name,
                        &mut resolved_export_table,
                    );
                }
            }
            _ => {}
        }
    }

    let re_export_start = Instant::now();
    for statement in &parsed_file.statements {
        let ParsedStatement::ExportDeclaration(export) = statement else {
            continue;
        };
        let ParsedExportDeclaration::All {
            module_specifier,
            module_specifier_span,
            ..
        } = export.as_ref()
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
            &parsed_file.file_name,
        ) else {
            record_unresolved_external_module(ctx, module_specifier);
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

        ctx.set_file_name(parsed_file.file_name.clone());

        let resolved_type_declarations =
            Arc::make_mut(&mut resolved_export_table.type_declarations);
        // Payload sharing (`insert_shared_from`) was tried here and reverted:
        // collapsing the re-exported clone into the source payload changes
        // which first-wins expansion later consumers observe (zod message
        // drift). Re-export entries keep their per-table copies.
        for (name, declaration) in target_export_table.type_declarations.iter() {
            if resolved_type_declarations.get(name.as_ref()).is_none() {
                let _ = resolved_type_declarations.insert(name.clone(), declaration.clone());
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

    if let Some(slot) = resolving.get_mut(file_index) {
        *slot = false;
    }
    resolved_export_table.namespace_export_object_type =
        Some(compute_namespace_export_object_type(&resolved_export_table));
    if let Some(slot) = resolved_module_export_tables.get_mut(file_index) {
        *slot = Some(resolved_export_table.clone_with_reason(TypeCopyReason::ModuleExport));
    }
    Some(resolved_export_table)
}
