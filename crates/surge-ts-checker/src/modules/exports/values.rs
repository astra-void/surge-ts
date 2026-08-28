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
    fn walk(
        statement: &ParsedStatement,
        exportable_values: &mut SymbolTable,
        ctx: &CheckerContext,
    ) {
        match statement {
            ParsedStatement::VariableDeclaration(variable) => {
                if exportable_values.get_own_shared(&variable.name).is_none() {
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
                    walk(declaration.as_ref(), exportable_values, ctx);
                }
            }
            ParsedStatement::NamespaceDeclaration(namespace) => {
                if exportable_values.get_own(&namespace.name).is_none() {
                    let _ = exportable_values.insert(
                        namespace.name.clone(),
                        SymbolInfo {
                            ty: namespace_value_object_type(namespace),
                            kind: SymbolKind::Const,
                            function_signature: None,
                        },
                    );
                }
                // The qualified `ns.member` keys are part of the name surface a
                // thin round must reproduce: a consumer that finds the key with a
                // thin `Unknown` type degrades (and re-resolves later), while one
                // that misses it entirely reads the namespace object's permissive
                // `any` member and *caches* that answer.
                thin_namespace_member_names(namespace, &namespace.name, exportable_values, ctx);
            }
            _ => {}
        }
    }

    /// The qualified members a thin round publishes. Their *shape* stays
    /// permissive (no annotation is resolved here), but the parsed signature is
    /// carried so a consumer bound against this round still instantiates the
    /// member correctly — the import bindings taken here are what the check
    /// phase reads, so a thin round that omitted them silently pinned every
    /// consumer to the namespace object's `any` member.
    fn thin_namespace_member_names(
        namespace: &surge_ts_syntax::ParsedNamespaceDeclaration,
        prefix: &str,
        exportable_values: &mut SymbolTable,
        ctx: &CheckerContext,
    ) {
        for statement in &namespace.statements {
            let inner = peel_exported_statement(statement);
            let (name, kind, signature) = match inner {
                ParsedStatement::FunctionDeclaration(function) => {
                    if !publishable_member_signature(
                        &function.type_parameters,
                        function
                            .parameters
                            .iter()
                            .filter_map(|p| p.declared_type.as_ref()),
                        function.return_type.as_ref(),
                        namespace,
                        ctx,
                    ) {
                        continue;
                    }
                    (
                        function.name.as_str(),
                        SymbolKind::Function,
                        crate::checks::function::namespace_member_signature_info(
                            &function.type_parameters,
                            &function.parameters,
                            function.return_type.as_ref(),
                            &ctx.file_name,
                            prefix,
                        ),
                    )
                }
                ParsedStatement::VariableDeclaration(variable) => {
                    let Some(surge_ts_syntax::ParsedExpression::ArrowFunction(arrow)) =
                        variable.initializer.as_ref()
                    else {
                        continue;
                    };
                    if variable.declared_type.is_some()
                        || !publishable_member_signature(
                            &arrow.type_parameters,
                            arrow
                                .parameters
                                .iter()
                                .filter_map(|p| p.declared_type.as_ref()),
                            arrow.return_type.as_ref(),
                            namespace,
                            ctx,
                        )
                    {
                        continue;
                    }
                    (
                        variable.name.as_str(),
                        SymbolKind::Const,
                        crate::checks::function::namespace_member_signature_info(
                            &arrow.type_parameters,
                            &arrow.parameters,
                            arrow.return_type.as_ref(),
                            &ctx.file_name,
                            prefix,
                        ),
                    )
                }
                ParsedStatement::NamespaceDeclaration(inner_namespace) => {
                    let inner_prefix = format!("{prefix}.{}", inner_namespace.name);
                    thin_namespace_member_names(
                        inner_namespace,
                        &inner_prefix,
                        exportable_values,
                        ctx,
                    );
                    continue;
                }
                _ => continue,
            };
            let key = format!("{prefix}.{name}");
            if exportable_values.get_own_shared(&key).is_none() {
                let _ = exportable_values.insert(
                    key,
                    SymbolInfo {
                        ty: Type::Function(surge_ts_types::FunctionType::new(
                            vec![Type::Any],
                            Type::Any,
                            true,
                            0,
                        )),
                        kind,
                        function_signature: Some(signature),
                    },
                );
            }
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
    let merging_namespaces = merging_namespace_value_members(statements);
    for statement in statements {
        if is_merging_namespace_statement(statement, &merging_namespaces) {
            continue;
        }
        walk(statement, &mut exportable_values, ctx);
    }
    apply_merging_namespace_value_members(&merging_namespaces, &mut exportable_values);
    exportable_values
}

/// Namespaces in `statements` that declaration-merge with a same-named
/// variable/function/class in the same list, paired with their accumulated
/// value members (merged across every block of the name, in first-appearance
/// order).
///
/// TypeScript merges `namespace X` into a same-named value declaration; surge
/// used to let whichever came first win outright, so `@types/node`'s
/// `namespace path { interface PlatformPath … } const path: path.PlatformPath`
/// bound `path` to the namespace's *empty* value object and collapsed every
/// `path.resolve(…)` to a missing property on `{}`.
fn merging_namespace_value_members(
    statements: &[ParsedStatement],
) -> Vec<(String, surge_ts_types::PropertyMap)> {
    let declares_namespace = statements.iter().any(|statement| {
        matches!(
            peel_exported_statement(statement),
            ParsedStatement::NamespaceDeclaration(_)
        )
    });
    if !declares_namespace {
        return Vec::new();
    }

    let mut value_names = surge_ts_types::fx::FxHashSet::default();
    for statement in statements {
        match peel_exported_statement(statement) {
            ParsedStatement::VariableDeclaration(variable) => {
                value_names.insert(variable.name.as_str());
            }
            ParsedStatement::FunctionDeclaration(function) => {
                value_names.insert(function.name.as_str());
            }
            ParsedStatement::ClassDeclaration(class) => {
                value_names.insert(class.name.as_str());
            }
            _ => {}
        }
    }

    if value_names.is_empty() {
        return Vec::new();
    }

    let mut merged: Vec<(String, surge_ts_types::PropertyMap)> = Vec::new();
    for statement in statements {
        let ParsedStatement::NamespaceDeclaration(namespace) = peel_exported_statement(statement)
        else {
            continue;
        };
        if !value_names.contains(namespace.name.as_str()) {
            continue;
        }
        let index = match merged.iter().position(|(name, _)| name == &namespace.name) {
            Some(index) => index,
            None => {
                merged.push((
                    namespace.name.clone(),
                    surge_ts_types::PropertyMap::default(),
                ));
                merged.len() - 1
            }
        };
        fill_namespace_value_properties(namespace, &mut merged[index].1);
    }
    merged
}

fn peel_exported_statement(statement: &ParsedStatement) -> &ParsedStatement {
    match statement {
        ParsedStatement::ExportDeclaration(export) => {
            if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                peel_exported_statement(declaration.as_ref())
            } else {
                statement
            }
        }
        other => other,
    }
}

fn is_merging_namespace_statement(
    statement: &ParsedStatement,
    merging_namespaces: &[(String, surge_ts_types::PropertyMap)],
) -> bool {
    if merging_namespaces.is_empty() {
        return false;
    }
    let ParsedStatement::NamespaceDeclaration(namespace) = peel_exported_statement(statement)
    else {
        return false;
    };
    merging_namespaces
        .iter()
        .any(|(name, _)| name == &namespace.name)
}

/// Overlays each merging namespace's value members onto the value symbol the
/// walk produced, without displacing members the value already carries.
/// The value symbol is left alone when the namespace contributes no value
/// members (the `@types/node` shape: the namespace holds types only).
fn apply_merging_namespace_value_members(
    merging_namespaces: &[(String, surge_ts_types::PropertyMap)],
    exportable_values: &mut SymbolTable,
) {
    for (name, members) in merging_namespaces {
        let Some(symbol) = exportable_values.get_shared(name) else {
            // The value declaration bound nothing (an unsupported binding form);
            // fall back to the namespace object so the name stays a value.
            let _ = exportable_values.insert(
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(crate::arena::alloc_object_type(members.clone(), None)),
                    kind: SymbolKind::Const,
                    function_signature: None,
                },
            );
            continue;
        };

        if members.is_empty() {
            continue;
        }

        let merged_type = match &symbol.ty {
            Type::Object(object) => Type::Object(object_with_namespace_members(object, members)),
            Type::Function(function) => Type::Object(
                crate::arena::alloc_object_type(members.clone(), None)
                    .with_call_signature(function.clone()),
            ),
            Type::Unknown => Type::Object(crate::arena::alloc_object_type(members.clone(), None)),
            _ => continue,
        };

        let _ = exportable_values.insert(
            name.clone(),
            SymbolInfo {
                ty: merged_type,
                kind: symbol.kind,
                function_signature: symbol.function_signature.clone(),
            },
        );
    }
}

fn object_with_namespace_members(
    object: &surge_ts_types::ObjectType,
    members: &surge_ts_types::PropertyMap,
) -> surge_ts_types::ObjectType {
    let mut properties = (*object.properties).clone();
    for (member_name, member) in members.iter() {
        properties
            .entry(member_name.clone())
            .or_insert_with(|| member.clone());
    }

    let mut merged = crate::arena::alloc_object_type(
        properties,
        object.string_index_type.as_ref().map(|ty| (**ty).clone()),
    );
    merged.alias_name = object.alias_name.clone();
    merged.alias_id = object.alias_id.clone();
    merged.construct_signature = object.construct_signature.clone();
    merged.call_signature = object.call_signature.clone();
    merged.is_intersection = object.is_intersection;
    merged.synthetic_open_index = object.synthetic_open_index;
    merged
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
        //
        // Source files deliberately do *not* share it: an interleaved A/B on zod
        // measured +180 MB peak RSS (585 -> 766 MB) for eight diagnostics, so the
        // references those files leave behind are handled where they are read
        // instead — a receiver that peels to the sentinel reports nothing.
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

    let merging_namespaces = merging_namespace_value_members(statements);
    for statement in statements {
        if is_merging_namespace_statement(statement, &merging_namespaces) {
            continue;
        }
        collect_exportable_value_symbols_from_statement(
            statement,
            &mut exportable_values,
            &mut shadow_ctx,
            !library_file,
        );
    }
    apply_merging_namespace_value_members(&merging_namespaces, &mut exportable_values);

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

pub(crate) fn annotation_contains_typeof(annotation: &surge_ts_syntax::ParsedType) -> bool {
    use surge_ts_syntax::ParsedType;
    match annotation {
        ParsedType::TypeOf(_) => true,
        ParsedType::Array(element) | ParsedType::KeyOf(element) => {
            annotation_contains_typeof(element)
        }
        ParsedType::Tuple(elements)
        | ParsedType::Union(elements)
        | ParsedType::Intersection(elements) => elements.iter().any(annotation_contains_typeof),
        ParsedType::Object(object) => {
            object
                .properties
                .iter()
                .any(|property| annotation_contains_typeof(&property.ty))
                || object
                    .call_signature
                    .as_ref()
                    .is_some_and(|signature| function_type_contains_typeof(signature))
        }
        ParsedType::Function(function) => function_type_contains_typeof(function),
        ParsedType::Named(named) => named.type_arguments.iter().any(annotation_contains_typeof),
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
                // Deliberately a parent-traversing lookup, unlike the sibling
                // guards: switching this one to `get_own_shared` measured +26
                // false positives on tRPC (TS2304 on names that are declared,
                // TS2339 on `path.join`) with no offsetting win, so the lazy
                // annotation path depends on seeing the global. Tracked
                // separately from the same-name clobber the other guards fix.
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
            let existing_symbol = exportable_values.get_own_shared(&variable.name);
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
            if exportable_values.get_own(&namespace.name).is_none() {
                let _ = exportable_values.insert(
                    namespace.name.clone(),
                    SymbolInfo {
                        ty: namespace_value_object_type(namespace),
                        kind: SymbolKind::Const,
                        function_signature: None,
                    },
                );
            }
            collect_namespace_member_value_symbols(
                namespace,
                &namespace.name,
                exportable_values,
                ctx,
            );
        }
        _ => {}
    }
}

/// Whether a namespace member's signature can be re-resolved from a *consumer's*
/// site. Instantiation re-resolves the written annotations under the declaring
/// file's module scope, which does not see the namespace's own body: zod's
/// `util.assertEqual = <A, B>(_: AssertEqual<A, B>) => void` names a
/// namespace-local alias, and publishing its signature turns every call into a
/// `TS2304` for that alias. Only self-contained signatures — every named type is
/// either one of the member's own type parameters or a declaration visible at
/// file scope — get one; the rest keep the pre-existing permissive behavior.
fn publishable_member_signature<'a>(
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
    parameter_types: impl Iterator<Item = &'a surge_ts_syntax::ParsedType>,
    return_type: Option<&surge_ts_syntax::ParsedType>,
    namespace: &ParsedNamespaceDeclaration,
    ctx: &CheckerContext,
) -> bool {
    // Only *generic* members need the qualified entry: a non-generic one is
    // already callable through the namespace object, and publishing every member
    // of every ambient namespace measured +300 MB peak RSS on tRPC.
    // The one exception is a type-predicate return (`node is ImportDeclaration`):
    // the namespace object's permissive member drops the predicate, so every
    // branch guarded by `ts.isImportDeclaration(node)` loses its narrowing.
    let is_type_predicate = matches!(return_type, Some(surge_ts_syntax::ParsedType::Predicate(_)));
    if type_parameters.is_empty() && !is_type_predicate {
        return false;
    }
    let mut scan = SignatureNameScan::default();
    for parameter_type in parameter_types {
        collect_signature_type_names(parameter_type, &mut scan);
    }
    if let Some(return_type) = return_type {
        collect_signature_type_names(return_type, &mut scan);
    }
    for type_parameter in type_parameters {
        if let Some(constraint) = type_parameter.constraint.as_ref() {
            collect_signature_type_names(constraint, &mut scan);
        }
        if let Some(default_type) = type_parameter.default_type.as_ref() {
            collect_signature_type_names(default_type, &mut scan);
        }
    }
    if scan.has_type_query {
        return false;
    }
    scan.names.iter().all(|name| {
        type_parameters
            .iter()
            .any(|type_parameter| type_parameter.name == **name)
            || scan.bound.iter().any(|bound| bound == name)
            || ctx.lookup_type_declaration(name).is_some()
            || namespace_exports_sibling_type(namespace, name, ctx)
    })
}

/// Whether a bare name in a namespace member's signature names a type the
/// namespace *exports* — `React.useState`'s return `Dispatch<SetStateAction<S>>`
/// names `React.Dispatch`, stored under the qualified key, and instantiation
/// resolves it there through the member's namespace prefix. Such a signature is
/// still self-contained.
///
/// A namespace-private type is not: zod's
/// `util.assertEqual = <A, B>(_: AssertEqual<A, B>) => void` names an unexported
/// `AssertEqual`, which no consumer can see. In a declaration file every member
/// of an ambient namespace is exported, so the `export` keyword is not required
/// there.
fn namespace_exports_sibling_type(
    namespace: &ParsedNamespaceDeclaration,
    name: &str,
    ctx: &CheckerContext,
) -> bool {
    if name.contains('.') {
        return false;
    }
    let ambient = [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|extension| {
            ctx.file_name.len() >= extension.len()
                && ctx.file_name[ctx.file_name.len() - extension.len()..]
                    .eq_ignore_ascii_case(extension)
        });
    namespace.statements.iter().any(|statement| {
        let (exported, inner) = match statement {
            ParsedStatement::ExportDeclaration(export) => {
                match export.as_ref() {
                    ParsedExportDeclaration::Statement { declaration, .. } => {
                        (true, declaration.as_ref())
                    }
                    _ => return false,
                }
            }
            other => (ambient, other),
        };
        if !exported {
            return false;
        }
        match inner {
            ParsedStatement::TypeAliasDeclaration(alias) => alias.name == name,
            ParsedStatement::InterfaceDeclaration(interface) => interface.name == name,
            ParsedStatement::ClassDeclaration(class) => class.name == name,
            _ => false,
        }
    })
}

/// Names referenced by a signature, split into free references and the ones a
/// mapped type's key or an `infer` capture binds locally.
#[derive(Default)]
struct SignatureNameScan<'a> {
    names: Vec<&'a str>,
    bound: Vec<&'a str>,
    has_type_query: bool,
}

fn collect_signature_type_names<'a>(
    ty: &'a surge_ts_syntax::ParsedType,
    scan: &mut SignatureNameScan<'a>,
) {
    use surge_ts_syntax::ParsedType;
    match ty {
        ParsedType::Named(named) => {
            scan.names.push(named.name.as_str());
            for argument in &named.type_arguments {
                collect_signature_type_names(argument, scan);
            }
        }
        ParsedType::TypeOf(_) => scan.has_type_query = true,
        ParsedType::Infer(name) => scan.bound.push(name.as_str()),
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => {
            collect_signature_type_names(inner, scan);
        }
        ParsedType::Tuple(members)
        | ParsedType::Union(members)
        | ParsedType::Intersection(members) => {
            for member in members.iter() {
                collect_signature_type_names(member, scan);
            }
        }
        ParsedType::Object(object) => {
            for property in &object.properties {
                collect_signature_type_names(&property.ty, scan);
            }
        }
        ParsedType::Function(function) => {
            for parameter in &function.parameters {
                collect_signature_type_names(&parameter.ty, scan);
            }
            collect_signature_type_names(&function.return_type, scan);
        }
        ParsedType::IndexedAccess(indexed) => {
            collect_signature_type_names(&indexed.object_type, scan);
            collect_signature_type_names(&indexed.index_type, scan);
        }
        ParsedType::Mapped(mapped) => {
            // The mapped key (`[k in …]`) binds `k` over the value type.
            scan.bound.push(mapped.key_name.as_str());
            collect_signature_type_names(&mapped.constraint, scan);
            collect_signature_type_names(&mapped.value_type, scan);
        }
        ParsedType::Conditional(conditional) => {
            collect_signature_type_names(&conditional.check_type, scan);
            collect_signature_type_names(&conditional.extends_type, scan);
            collect_signature_type_names(&conditional.true_type, scan);
            collect_signature_type_names(&conditional.false_type, scan);
        }
        _ => {}
    }
}

/// Publishes a namespace's value members under qualified `ns.member` keys, the
/// value-side twin of the `ns.Member` type exports. The namespace object itself
/// stays permissive (member *set* only), so a call through it would otherwise
/// lose the member's arity, return type, and — for a generic member like zod's
/// `util.arrayToEnum` — any chance of inferring its type arguments. The
/// qualified entry carries the real signature, which the property-call path
/// consults by name.
pub(crate) fn collect_namespace_member_value_symbols(
    namespace: &ParsedNamespaceDeclaration,
    prefix: &str,
    exportable_values: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    for statement in &namespace.statements {
        let inner = peel_exported_statement(statement);
        match inner {
            ParsedStatement::FunctionDeclaration(function) => {
                if !publishable_member_signature(
                    &function.type_parameters,
                    function
                        .parameters
                        .iter()
                        .filter_map(|p| p.declared_type.as_ref()),
                    function.return_type.as_ref(),
                    namespace,
                    ctx,
                ) {
                    continue;
                }
                let key = format!("{prefix}.{}", function.name);
                if exportable_values.get_own(&key).is_some() {
                    continue;
                }
                let function_type = map_member_signature_in_namespace_scope(
                    &function.parameters,
                    function.return_type.as_ref(),
                    &function.type_parameters,
                    prefix,
                    ctx,
                );
                let function_signature = crate::checks::function::namespace_member_signature_info(
                    &function.type_parameters,
                    &function.parameters,
                    function.return_type.as_ref(),
                    &ctx.file_name,
                    prefix,
                );
                let _ = exportable_values.insert(
                    key,
                    SymbolInfo {
                        ty: Type::Function(function_type),
                        kind: SymbolKind::Function,
                        function_signature: Some(function_signature),
                    },
                );
            }
            ParsedStatement::VariableDeclaration(variable) => {
                let Some(surge_ts_syntax::ParsedExpression::ArrowFunction(arrow)) =
                    variable.initializer.as_ref()
                else {
                    continue;
                };
                if variable.declared_type.is_some()
                    || !publishable_member_signature(
                        &arrow.type_parameters,
                        arrow
                            .parameters
                            .iter()
                            .filter_map(|p| p.declared_type.as_ref()),
                        arrow.return_type.as_ref(),
                        namespace,
                        ctx,
                    )
                {
                    continue;
                }
                let key = format!("{prefix}.{}", variable.name);
                if exportable_values.get_own(&key).is_some() {
                    continue;
                }
                let function_type = map_member_signature_in_namespace_scope(
                    &arrow.parameters,
                    arrow.return_type.as_ref(),
                    &arrow.type_parameters,
                    prefix,
                    ctx,
                );
                let function_signature = crate::checks::function::namespace_member_signature_info(
                    &arrow.type_parameters,
                    &arrow.parameters,
                    arrow.return_type.as_ref(),
                    &ctx.file_name,
                    prefix,
                );
                let _ = exportable_values.insert(
                    key,
                    SymbolInfo {
                        ty: Type::Function(function_type),
                        kind: match variable.kind {
                            surge_ts_syntax::ParsedVariableKind::Var => SymbolKind::Var,
                            surge_ts_syntax::ParsedVariableKind::Let => SymbolKind::Let,
                            surge_ts_syntax::ParsedVariableKind::Const => SymbolKind::Const,
                        },
                        function_signature: Some(function_signature),
                    },
                );
            }
            ParsedStatement::NamespaceDeclaration(inner_namespace) => {
                let inner_prefix = format!("{prefix}.{}", inner_namespace.name);
                collect_namespace_member_value_symbols(
                    inner_namespace,
                    &inner_prefix,
                    exportable_values,
                    ctx,
                );
            }
            _ => {}
        }
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
        // `declare namespace N { export { a, b } }` re-exports declarations from
        // the enclosing file as namespace members (the shape Prisma's runtime
        // `Extensions` uses). The referenced declarations are not in this body,
        // so the members are permissive — enough for `N.a` to resolve instead of
        // reporting a missing property on an empty object.
        if let ParsedStatement::ExportDeclaration(export) = statement
            && let ParsedExportDeclaration::Named {
                specifiers,
                module_specifier: None,
                is_type_only: false,
                ..
            } = export.as_ref()
        {
            for specifier in specifiers {
                if specifier.is_type_only {
                    continue;
                }
                properties.insert(
                    specifier.exported_name.as_str().into(),
                    ObjectProperty::required(Type::Any),
                );
            }
            continue;
        }

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

/// Maps a member's written signature in the namespace's own scope: a bare
/// sibling name (`Dispatch` inside `React.useState`) is stored under a qualified
/// key, and one that stays unresolvable belongs to a surface only partially
/// modelled here, so it degrades to `unknown` instead of cascading TS2304. The
/// publishability decision deliberately runs *outside* this scope — see
/// [`publishable_member_signature`].
fn map_member_signature_in_namespace_scope(
    parameters: &[surge_ts_syntax::ParsedFunctionParameter],
    return_type: Option<&surge_ts_syntax::ParsedType>,
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
    prefix: &str,
    ctx: &mut CheckerContext,
) -> surge_ts_types::FunctionType {
    ctx.namespace_member_resolution_depth += 1;
    ctx.namespace_member_prefix_stack.push(prefix.to_string());
    let function_type = crate::checks::function::map_function_signature(
        parameters,
        return_type,
        type_parameters,
        None,
        ctx,
    );
    ctx.namespace_member_prefix_stack.pop();
    ctx.namespace_member_resolution_depth -= 1;
    function_type
}
