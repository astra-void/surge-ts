use super::*;

/// Superseded analysis rounds (the round-0 type-binding pass and the
/// preliminary module-analysis round) collect exportable value symbols THIN:
/// same symbol-name surface, but declared values carry `Unknown` instead of an
/// eagerly resolved annotation/initializer type. Sound because nothing the
/// final round bakes into output reads those intermediate value types: the
/// check phase consumes the FINAL round's full-fidelity export tables plus
/// `module_local_values_by_file` (populated after the final round), and the
/// preliminary value types only ever bootstrapped the name surface. Measured
/// on tRPC: eliminates ~2s of duplicated annotation resolution
/// (`map_parsed_type_with_substitution` was ~75% of value collection),
/// byte-identical across trpc/zod/ky/ofetch × jobs auto/8/1.
/// `SURGE_THIN_PRELIM=0` restores the old eager behavior for A/B.
pub(crate) fn thin_prelim_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_THIN_PRELIM").as_deref() != Ok("0"))
}

/// The thin variant of [`collect_exportable_value_symbols`]: same symbol name
/// surface (variables degrade to `Unknown`, namespace value objects keep their
/// permissive member sets), no shadow context, no annotation resolution, no
/// initializer inference. See [`thin_prelim_enabled`] for the soundness
/// argument.
fn collect_exportable_value_symbols_thin(
    statements: &[ParsedStatement],
    local_symbols: &SymbolTable,
    ctx: &CheckerContext,
) -> SymbolTable {
    fn walk(statement: &ParsedStatement, exportable_values: &mut SymbolTable) {
        match statement {
            ParsedStatement::VariableDeclaration(variable) => {
                if exportable_values.get_shared(&variable.name).is_none() {
                    let kind = match variable.kind {
                        surge_ts_syntax::ParsedVariableKind::Var => SymbolKind::Var,
                        surge_ts_syntax::ParsedVariableKind::Let => SymbolKind::Let,
                        surge_ts_syntax::ParsedVariableKind::Const => SymbolKind::Const,
                    };
                    let _ = exportable_values.insert(
                        variable.name.clone(),
                        SymbolInfo {
                            ty: Type::Unknown,
                            kind,
                            function_signature: None,
                        },
                    );
                }
            }
            ParsedStatement::ExportDeclaration(export) => {
                if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                    walk(declaration.as_ref(), exportable_values);
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

    let mut exportable_values = SymbolTable::new();
    for (name, symbol) in local_symbols.iter_shared() {
        let _ = exportable_values.insert_shared(name.clone(), symbol.clone());
    }
    let mut exportable_values = exportable_values.with_parent_fallback(Arc::new(
        ctx.ambient_global_symbols
            .clone_with_reason(TypeCopyReason::ModuleExport),
    ));
    for statement in statements {
        walk(statement, &mut exportable_values);
    }
    exportable_values
}

pub(crate) fn collect_exportable_value_symbols(
    statements: &[ParsedStatement],
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    imported_symbols: Option<&SymbolTable>,
    ctx: &CheckerContext,
) -> SymbolTable {
    if thin_prelim_enabled() && ctx.thin_superseded_value_collection {
        return collect_exportable_value_symbols_thin(statements, local_symbols, ctx);
    }
    let mut file_kinds = surge_ts_types::fx::FxHashMap::default();
    file_kinds.insert(ctx.file_name.clone(), FileKind::RootSource);
    let mut shadow_ctx = CheckerContext::new_with_shared_options(
        ctx.file_name.clone(),
        Arc::clone(&ctx.options),
        file_kinds,
    );
    shadow_ctx.timings = ctx.timings.clone();
    // Environment identity must be content-stable: the shadow inherits the
    // deterministic stage counter and attempt tag, and its fresh memo map gets
    // a shadow-window ordinal so its environments never collide with the
    // module body's (ordinals 0/1).
    shadow_ctx.resolution_stage_counter = ctx.resolution_stage_counter;
    shadow_ctx.environment_attempt = ctx.environment_attempt;
    shadow_ctx.replace_resolved_named_types(2);
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
    shadow_ctx.lazy_library_value_annotations = library_file && lazy_dts_values_enabled();
    if shadow_ctx.lazy_library_value_annotations {
        // Lazy value-annotation references capture their declaration
        // environment; the shadow's own store dies with the shadow, so the
        // capture must intern into the caller's persistent store or every
        // force degrades to `Unknown` (`checker_context()` -> None).
        shadow_ctx.declaration_environment_store = ctx.declaration_environment_store.clone();
    }
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

/// Library `.d.ts` value annotations become lazy references (mapped on first
/// read, unpeeled — matching eager `map_parsed_type` output shape) instead of
/// being eagerly mapped once per analysis round. tRPC: −1s wall. Soundness
/// pillars: the shadow shares the caller's persistent declaration-environment
/// store (a shadow-owned store dies with the shadow and every force degrades
/// to `Unknown`), typeof-bearing annotations stay eager (their query resolves
/// against the collection-time working symbol table, which environment
/// capture deliberately drops), and primitives stay eager (deferring them
/// only exposes unforced references to structural variant matches).
/// `SURGE_LAZY_DTS_VALUES=0` restores the old eager behavior for A/B.
fn lazy_dts_values_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_LAZY_DTS_VALUES").as_deref() != Ok("0"))
}

/// Whether a library value annotation is worth deferring: structural shapes
/// and named references (where alias-expansion cost lives). Primitives,
/// literals, and keyword types map in nanoseconds — deferring them only
/// exposes an unforced reference to structural checks (e.g. comparison
/// overlap) that inspect types without peeling. Annotations containing
/// `typeof` anywhere stay eager: the eager path resolves the query against
/// the collection-time working symbol table (same-file values collected so
/// far), which the captured declaration environment deliberately drops, and
/// `module_local_values_by_file` is not populated yet when the capture is
/// taken during the final analysis round.
fn defer_value_annotation(annotation: &surge_ts_syntax::ParsedType) -> bool {
    use surge_ts_syntax::ParsedType;
    match annotation {
        ParsedType::Object(_)
        | ParsedType::Tuple(_)
        | ParsedType::Union(_)
        | ParsedType::Intersection(_)
        | ParsedType::Function(_)
        | ParsedType::KeyOf(_)
        | ParsedType::IndexedAccess(_)
        | ParsedType::Mapped(_)
        | ParsedType::Conditional(_)
        | ParsedType::TemplateLiteral(_)
        | ParsedType::Named(_) => !annotation_contains_typeof(annotation),
        ParsedType::Array(element) => defer_value_annotation(element),
        ParsedType::TypeOf(_) => false,
        _ => false,
    }
}

fn annotation_contains_typeof(annotation: &surge_ts_syntax::ParsedType) -> bool {
    use surge_ts_syntax::ParsedType;
    match annotation {
        ParsedType::TypeOf(_) => true,
        ParsedType::Array(element) | ParsedType::KeyOf(element) => {
            annotation_contains_typeof(element)
        }
        ParsedType::Tuple(elements)
        | ParsedType::Union(elements)
        | ParsedType::Intersection(elements) => {
            elements.iter().any(annotation_contains_typeof)
        }
        ParsedType::Object(object) => {
            object
                .properties
                .iter()
                .any(|property| annotation_contains_typeof(&property.ty))
                || object.call_signature.as_ref().is_some_and(|signature| {
                    function_type_contains_typeof(signature)
                })
        }
        ParsedType::Function(function) => function_type_contains_typeof(function),
        ParsedType::Named(named) => named
            .type_arguments
            .iter()
            .any(annotation_contains_typeof),
        ParsedType::IndexedAccess(indexed) => {
            annotation_contains_typeof(&indexed.object_type)
                || annotation_contains_typeof(&indexed.index_type)
        }
        ParsedType::Mapped(mapped) => {
            annotation_contains_typeof(&mapped.constraint)
                || annotation_contains_typeof(&mapped.value_type)
        }
        ParsedType::Conditional(conditional) => {
            annotation_contains_typeof(&conditional.check_type)
                || annotation_contains_typeof(&conditional.extends_type)
                || annotation_contains_typeof(&conditional.true_type)
                || annotation_contains_typeof(&conditional.false_type)
        }
        ParsedType::TemplateLiteral(template) => template
            .interpolations
            .iter()
            .any(annotation_contains_typeof),
        _ => false,
    }
}

fn function_type_contains_typeof(function: &surge_ts_syntax::ParsedFunctionType) -> bool {
    function
        .parameters
        .iter()
        .any(|parameter| annotation_contains_typeof(&parameter.ty))
        || annotation_contains_typeof(&function.return_type)
        || function.type_parameters.iter().any(|parameter| {
            parameter
                .constraint
                .as_ref()
                .is_some_and(annotation_contains_typeof)
                || parameter
                    .default_type
                    .as_ref()
                    .is_some_and(annotation_contains_typeof)
        })
}

pub(crate) fn collect_exportable_value_symbols_from_statement(
    statement: &ParsedStatement,
    exportable_values: &mut SymbolTable,
    ctx: &mut CheckerContext,
    check_initializers: bool,
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            if ctx.lazy_library_value_annotations
                && variable.initializer.is_none()
                && variable
                    .declared_type
                    .as_ref()
                    .is_some_and(defer_value_annotation)
                && let Some(annotation) = variable.declared_type.clone()
            {
                if exportable_values.get_shared(&variable.name).is_none() {
                    let kind = match variable.kind {
                        surge_ts_syntax::ParsedVariableKind::Var => SymbolKind::Var,
                        surge_ts_syntax::ParsedVariableKind::Let => SymbolKind::Let,
                        surge_ts_syntax::ParsedVariableKind::Const => SymbolKind::Const,
                    };
                    let ty = crate::infer::make_lazy_value_annotation_reference(
                        ctx,
                        &variable.name,
                        variable.name_span.map_or(0, |span| span.start),
                        annotation,
                    );
                    let _ = exportable_values.insert(
                        variable.name.clone(),
                        SymbolInfo {
                            ty,
                            kind,
                            function_signature: None,
                        },
                    );
                }
                return;
            }
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
            if let Some(filter) = crate::infer::types::cache::lazy_value_trace_filter()
                && variable.name.contains(filter)
                && let Some(symbol) = exportable_values.get_shared(&variable.name)
            {
                eprintln!(
                    "[lazy-value] EAGER {}@{} file={} ty={}",
                    variable.name,
                    variable.name_span.map_or(0, |span| span.start),
                    ctx.file_name,
                    crate::infer::types::cache::lazy_value_trace_shape(&symbol.ty),
                );
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
