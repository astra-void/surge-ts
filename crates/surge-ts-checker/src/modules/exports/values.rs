use super::*;

pub(crate) fn collect_exportable_value_symbols(
    statements: &[ParsedStatement],
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    imported_symbols: Option<&SymbolTable>,
    ctx: &CheckerContext,
) -> SymbolTable {
    let mut file_kinds = surge_ts_types::fx::FxHashMap::default();
    file_kinds.insert(ctx.file_name.clone(), FileKind::RootSource);
    let mut shadow_ctx = CheckerContext::new_with_shared_options(
        ctx.file_name.clone(),
        Arc::clone(&ctx.options),
        file_kinds,
    );
    shadow_ctx.timings = ctx.timings.clone();
    shadow_ctx.physical_interface_instantiations = ctx.physical_interface_instantiations.clone();
    shadow_ctx.physical_interface_declaration_templates =
        ctx.physical_interface_declaration_templates.clone();
    shadow_ctx.physical_interface_method_instantiations =
        ctx.physical_interface_method_instantiations.clone();
    shadow_ctx.physical_interface_overload_instantiations =
        ctx.physical_interface_overload_instantiations.clone();

    let _ = local_type_declarations;
    shadow_ctx.type_declarations = ctx.type_declarations.clone();
    // The caller's full type-resolution surface must travel into the shadow, or
    // an exported `const` whose annotation names an *imported* type (a generic
    // arrow component's `ControllerProps<T, N>` parameter, radix's
    // `Root: ForwardRefExoticComponent<CheckboxProps & …>`) resolves to
    // `unknown` and every consumer loses its signature. All Arc-shared,
    // read-only state. Initializer inference is gated off for library
    // declaration files instead (below): annotations there must still resolve,
    // but a live scope made the initializer walk fully expand library type
    // graphs for every dependency module on every binding pass (unnamed peak
    // RSS 8.5GB with both, 5.1GB with annotations only, 2.8GB with neither —
    // the last silently degrades every `ComponentProps<typeof Primitive.X>`).
    let library_file = ctx.is_library_scoped_file(&ctx.file_name);
    shadow_ctx.type_declaration_scope = ctx.type_declaration_scope.clone();
    shadow_ctx.ambient_global_type_declarations = ctx.ambient_global_type_declarations.clone();
    shadow_ctx.ambient_global_symbols = ctx
        .ambient_global_symbols
        .clone_with_reason(TypeCopyReason::ModuleExport);
    shadow_ctx.module_scope_by_file = ctx.module_scope_by_file.clone();
    shadow_ctx.module_local_values_by_file = ctx.module_local_values_by_file.clone();
    // The file's import bindings back `typeof <importedValue>` in annotations
    // (radix's `ComponentPropsWithoutRef<typeof Primitive.button>`); they are a
    // resolution fallback only, never inserted into the exportable set.
    if let Some(imported_symbols) = imported_symbols {
        shadow_ctx.module_value_fallback = Some(Arc::new(
            imported_symbols.clone_with_reason(TypeCopyReason::ModuleExport),
        ));
    }

    // The ambient globals (the lib `.d.ts` surface, ~1000 entries) are only a
    // read-only resolution backdrop here: the returned table is consulted via
    // `get`, never iterated, and the actual export entries are built into a fresh
    // table by the caller. Holding the globals as a `parent` fallback rather than
    // as the own map keeps each module's export-table build O(local symbols)
    // instead of deep-copying every global on the first local insert.
    let mut exportable_values = SymbolTable::new();
    for (name, symbol) in local_symbols.iter_shared() {
        let _ = exportable_values.insert_shared(name.clone(), symbol.clone());
    }
    let mut exportable_values = exportable_values.with_parent_fallback(Arc::new(
        ctx.ambient_global_symbols
            .clone_with_reason(TypeCopyReason::ModuleExport),
    ));

    for statement in statements {
        collect_exportable_value_symbols_from_statement(
            statement,
            &mut exportable_values,
            &mut shadow_ctx,
            !library_file,
        );
    }

    exportable_values
}

pub(crate) fn collect_exportable_value_symbols_from_statement(
    statement: &ParsedStatement,
    exportable_values: &mut SymbolTable,
    ctx: &mut CheckerContext,
    check_initializers: bool,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            let existing_symbol = exportable_values.get_shared(&variable.name);
            let _ = check_variable_declaration_with_symbols(
                variable.as_ref().clone(),
                exportable_values,
                ctx,
                VariableCheckOptions {
                    report_duplicate_let_const: false,
                    check_initializer: check_initializers,
                },
            );

            if let Some(existing_symbol) = existing_symbol {
                exportable_values.insert_shared(variable.name.clone(), existing_symbol);
            }
        }
        ParsedStatement::ExportDeclaration(export) => {
            if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                collect_exportable_value_symbols_from_statement(
                    declaration.as_ref(),
                    exportable_values,
                    ctx,
                    check_initializers,
                )
            }
        }
        ParsedStatement::NamespaceDeclaration(namespace) => {
            if exportable_values.get(&namespace.name).is_none() {
                let _ = exportable_values.insert(
                    namespace.name.clone(),
                    SymbolInfo {
                        ty: namespace_value_object_type(namespace),
                        kind: SymbolKind::Const,
                        function_signature: None,
                    },
                );
            }
        }
        _ => {}
    }
}

/// The value-side object type of a `declare namespace`: one property per value
/// member (functions, consts, classes, nested namespaces). Member types are kept
/// permissive (functions accept any arguments, everything else is `any`) so the
/// namespace's member *set* is precise — enabling TS2339 on real typos — without
/// re-resolving a partially modelled surface and cascading. Used to bind an
/// `export = <namespace>` value so `import * as Ns` exposes `Ns.member`.
pub(crate) fn namespace_value_object_type(namespace: &ParsedNamespaceDeclaration) -> Type {
    let mut properties = surge_ts_types::PropertyMap::default();
    fill_namespace_value_properties(namespace, &mut properties);
    Type::Object(crate::arena::alloc_object_type(properties, None))
}

/// Accumulate a `declare namespace`'s value members into `properties`. Split into
/// its own function so a namespace declared across multiple merged blocks (e.g.
/// roblox-ts's `math`, declared with `noise`/`clamp` in one file and the Lua math
/// surface in another) can be assembled into a single value object.
pub(crate) fn fill_namespace_value_properties(
    namespace: &ParsedNamespaceDeclaration,
    properties: &mut surge_ts_types::PropertyMap,
) {
    use surge_ts_types::{FunctionType, ObjectProperty};

    for statement in &namespace.statements {
        let inner = match statement {
            ParsedStatement::ExportDeclaration(export) => {
                if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                    declaration.as_ref()
                } else {
                    statement
                }
            }
            other => other,
        };

        match inner {
            ParsedStatement::FunctionDeclaration(function) => {
                properties.insert(
                    function.name.as_str().into(),
                    ObjectProperty::required(Type::Function(FunctionType::new(
                        vec![],
                        Type::Any,
                        true,
                        0,
                    ))),
                );
            }
            ParsedStatement::VariableDeclaration(variable) => {
                properties.insert(
                    variable.name.as_str().into(),
                    ObjectProperty::required(Type::Any),
                );
            }
            ParsedStatement::ClassDeclaration(class) => {
                properties.insert(
                    class.name.as_str().into(),
                    ObjectProperty::required(Type::Any),
                );
            }
            ParsedStatement::NamespaceDeclaration(inner_namespace) => {
                properties.insert(
                    inner_namespace.name.as_str().into(),
                    ObjectProperty::required(namespace_value_object_type(inner_namespace)),
                );
            }
            _ => {}
        }
    }
}
