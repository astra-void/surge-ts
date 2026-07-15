use super::*;

/// The module specifier of a `import * as <local_name>` declaration in this
/// file, if any — the binding a `export { <local_name> }` re-export refers to.
pub(super) fn namespace_import_module_specifier(
    parsed_file: &ParsedProgramFile,
    local_name: &str,
) -> Option<String> {
    parsed_file.statements.iter().find_map(|statement| {
        let ParsedStatement::ImportDeclaration(import) = statement else {
            return None;
        };
        match &import.kind {
            ParsedImportKind::Namespace {
                local_name: import_local_name,
                ..
            } if import_local_name == local_name => Some(import.module_specifier.clone()),
            _ => None,
        }
    })
}

/// Materializes a re-exported namespace's type members into the export table as
/// qualified `<exported_name>.<member>` keys, mirroring how consumers of
/// `import * as` see them through alias scope layers.
pub(super) fn copy_namespace_member_type_exports(
    target_export_table: &ModuleExportTable,
    exported_name: &str,
    resolved_export_table: &mut ModuleExportTable,
) {
    let type_declarations = Arc::make_mut(&mut resolved_export_table.type_declarations);
    for (key, declaration) in target_export_table.type_declarations.iter() {
        let qualified = format!("{exported_name}.{key}");
        if type_declarations.get(&qualified).is_none() {
            let _ = type_declarations.insert(qualified.as_str(), declaration.clone());
        }
    }
}

pub(crate) fn insert_namespace_export(
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

pub(crate) fn namespace_export_object_type(export_table: &ModuleExportTable) -> Type {
    if let Some(namespace_export_object_type) = &export_table.namespace_export_object_type {
        crate::program::record_module_export_borrowed_lookup_count();
        return namespace_export_object_type.clone();
    }

    compute_namespace_export_object_type(export_table)
}

pub(crate) fn compute_namespace_export_object_type(export_table: &ModuleExportTable) -> Type {
    crate::program::record_module_export_namespace_export_object_materialization_count();
    let mut properties = surge_ts_types::PropertyMap::new();
    let mut property_count = 0u64;

    for (name, symbol) in export_table.symbols.iter() {
        property_count += 1;
        properties.insert(
            name.to_string(),
            surge_ts_types::ObjectProperty::required(symbol.ty.clone()),
        );
    }

    if let Some(default_symbol) = &export_table.default_symbol {
        property_count += 1;
        properties.insert(
            "default".to_string(),
            surge_ts_types::ObjectProperty::required(default_symbol.ty.clone()),
        );
    }

    // `export = <namespace>` exposes the namespace object as the module's shape;
    // surface its members (e.g. `React.createContext`) on the namespace import.
    // The assigned value can sit behind a lazy nominal reference (an alias-typed
    // `export =`), which the bare `Type::Object` match silently dropped — peel it
    // so the namespace members still surface.
    if let Some(export_assignment_symbol) = &export_table.export_assignment_symbol {
        let peeled_assignment;
        let assignment_ty = match &export_assignment_symbol.ty {
            Type::Reference(_) => {
                peeled_assignment = export_assignment_symbol.ty.peeled();
                &peeled_assignment
            }
            other => other,
        };
        if let Type::Object(object) = assignment_ty {
            for (name, property) in object.properties.iter() {
                property_count += 1;
                properties
                    .entry(name.clone())
                    .or_insert_with(|| property.clone());
            }
        }
    }

    crate::program::record_module_export_namespace_export_object_property_count(property_count);

    Type::Object(crate::arena::alloc_object_type(properties, None))
}
