use super::*;

pub(crate) fn export_local_type_name(
    local_name: &str,
    exported_name: &str,
    name_span: &Option<TextSpan>,
    local_type_declarations: &TypeDeclarationTable,
    resolution_scope: Option<&Arc<TypeDeclarationScope>>,
    type_declarations: &mut TypeDeclarationTable,
    ctx: &mut CheckerContext,
) {
    // Read the local declaration through an arena-backed handle so re-export
    // binding hands `export_local_type_declaration` a borrow instead of a deep
    // clone. The rename/scope rewrite there still takes one owned copy; this
    // removes the redundant second clone this path previously paid per
    // re-exported type.
    let Some(handle) = local_type_declarations.get_handle(local_name) else {
        push_unresolved_export_diagnostic(ctx, local_name, *name_span);
        return;
    };

    export_local_type_declaration(
        handle.get(),
        exported_name,
        resolution_scope,
        type_declarations,
    );
}

pub(crate) fn export_local_type_declaration(
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

pub(crate) fn rename_type_declaration(
    declaration: TypeDeclarationInfo,
    exported_name: String,
) -> TypeDeclarationInfo {
    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            if alias.declared_name.is_none() {
                alias.declared_name = Some(alias.name.clone());
            }
            alias.name = exported_name;
            alias.cached_resolution_key = std::sync::OnceLock::new();
            alias.cached_alias_id = std::sync::OnceLock::new();
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            if interface.declared_name.is_none() {
                interface.declared_name = Some(interface.name.clone());
            }
            interface.name = exported_name;
            interface.cached_resolution_key = std::sync::OnceLock::new();
            interface.cached_alias_id = std::sync::OnceLock::new();
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

pub(crate) fn insert_type_export(
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

pub(crate) fn insert_unknown_type_import(
    type_declarations: &mut TypeDeclarationTable,
    local_name: &str,
    file_name: String,
    name_span: Option<TextSpan>,
) {
    let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo::new(
        local_name.to_string(),
        file_name,
        name_span,
        vec![],
        ParsedType::Unknown,
        None,
    ));

    let _ = type_declarations.insert(local_name.to_string(), declaration);
}

pub(crate) fn insert_unknown_value_import(local_name: &str, symbols: &mut SymbolTable) {
    let _ = symbols.insert(
        local_name.to_string(),
        SymbolInfo {
            ty: Type::Unknown,
            kind: SymbolKind::Var,
            function_signature: None,
        },
    );
}

pub(crate) fn lookup_type_export<'a>(
    export_table: &'a ModuleExportTable,
    local_name: &'a str,
) -> Option<&'a TypeDeclarationInfo> {
    crate::program::record_module_export_borrowed_lookup_count();
    export_table.type_declarations.get(local_name)
}

/// Copy an exported namespace's qualified type members (`ns.Member`) into the
/// importer's scope under the local binding name (`local.Member`), so qualified
/// type references through a named namespace import resolve.
/// Copies every `<imported_name>.<member>` qualified type export under the
/// local binding name. Returns whether any member was copied — a `true` means
/// the imported name exists as a (type-only) namespace even when it has no
/// direct type/value export entry of its own.
pub(crate) fn copy_qualified_type_exports(
    export_table: &ModuleExportTable,
    imported_name: &str,
    local_name: &str,
    type_declarations: &mut TypeDeclarationTable,
) -> bool {
    let prefix = format!("{imported_name}.");
    let mut copied_any = false;
    for (key, _) in export_table.type_declarations.iter() {
        if let Some(member) = key.as_str().strip_prefix(&prefix) {
            copied_any = true;
            let local_key = format!("{local_name}.{member}");
            let _ = type_declarations.insert_shared_from(
                &local_key,
                &export_table.type_declarations,
                key.as_str(),
            );
        }
    }
    copied_any
}

pub(crate) fn lookup_value_export(
    export_table: &ModuleExportTable,
    local_name: &str,
) -> Option<Arc<SymbolInfo>> {
    if local_name == "default" {
        return export_table.get_shared_value("default");
    }

    export_table.get_shared_value(local_name)
}
