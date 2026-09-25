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

thread_local! {
    static SOURCE_EXPORTS_SHARE_ENVIRONMENT_STORE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

pub(crate) const LAZY_INITIALIZER_ID_TAG: &str = "\u{0}lazy-initializer\u{0}";

thread_local! {
    static LAZY_INITIALIZERS_IN_PROGRESS: std::cell::RefCell<Vec<Arc<str>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static LAZY_READS_BEFORE_CHECK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Counts reads of a [`LazyInitializerValue`] answered before the check phase,
/// so a binding pass can tell a declaration it typed around that timing gap.
pub(crate) fn note_lazy_read_before_check() {
    LAZY_READS_BEFORE_CHECK.with(|reads| reads.set(reads.get().wrapping_add(1)));
}

fn lazy_reads_before_check() -> u64 {
    LAZY_READS_BEFORE_CHECK.with(std::cell::Cell::get)
}

/// A module variable the binding passes could not type: unannotated and left
/// at the sentinel, or annotated with a type that reads such a value
/// (`export const t: typeof router`). Typed when the check phase first reads it.
///
/// Go types a variable on demand (`getTypeOfVariableOrParameterOrProperty`,
/// checker.go:16844), so an initializer reading an imported value always sees
/// that value's type. surge publishes value exports from binding passes that run
/// before the values they import are typed: `export const procedure =
/// t.procedure`, with `t` built from an import, reached every importer as the
/// sentinel. The earlier passes already tried, so only the check phase infers,
/// against the declaring module's final value table (where `t` is itself one of
/// these); a cycle answers the sentinel where Go reports TS7022 and answers `any`.
struct LazyInitializerValue {
    id: Arc<str>,
    statement: ParsedStatement,
    name: String,
    file_name: Arc<str>,
    environment: crate::context::DeclarationEnvironmentHandle,
    creation_scope: Option<Arc<crate::symbols::TypeDeclarationScope>>,
    memo: std::sync::OnceLock<Type>,
}

impl surge_ts_types::ResolveReference for LazyInitializerValue {
    fn resolve(&self) -> Type {
        if let Some(resolved) = self.memo.get() {
            return resolved.clone();
        }
        // An answer given before the check phase, or to a re-entry, is a timing
        // gap; noting it keeps every enclosing expansion from interning it.
        if !crate::program::in_check_phase() {
            crate::program::note_expansion_degradation();
            note_lazy_read_before_check();
            return Type::Unknown;
        }
        let re_entered = LAZY_INITIALIZERS_IN_PROGRESS.with(|stack| {
            let mut stack = stack.borrow_mut();
            if stack.iter().any(|id| **id == *self.id) {
                return true;
            }
            stack.push(self.id.clone());
            false
        });
        if re_entered {
            crate::program::note_expansion_degradation();
            return Type::Unknown;
        }
        struct PopInProgress;
        impl Drop for PopInProgress {
            fn drop(&mut self) {
                LAZY_INITIALIZERS_IN_PROGRESS.with(|stack| {
                    stack.borrow_mut().pop();
                });
            }
        }
        let _pop = PopInProgress;
        let resolved = self.infer().unwrap_or(Type::Unknown);
        let _ = self.memo.set(resolved.clone());
        resolved
    }
}

impl LazyInitializerValue {
    fn infer(&self) -> Option<Type> {
        let mut ctx = self.environment.checker_context()?;
        ctx.set_file_name(self.file_name.to_string());
        if self.creation_scope.is_some() {
            ctx.type_declaration_scope = self.creation_scope.clone();
        }
        // Captured during module analysis, which runs without the per-file
        // scope map; the check phase's is what every declaration resolves in.
        if ctx.module_scope_by_file.is_empty()
            && let Some(scopes) = crate::program::program_module_scopes()
        {
            ctx.module_scope_by_file = scopes;
        }
        let values = self.environment.current_module_local_values(&self.file_name)?;
        let mut seed = values.as_ref().clone_with_reason(TypeCopyReason::ModuleExport);
        // Seeded, this very reference would be kept as the variable's
        // "existing" symbol instead of the inferred one.
        seed.remove(&self.name);
        // The sibling values it reads are read settled, as the check phase
        // installs a file's imports: a call through `router` must see the
        // builder, not the reference standing in for it.
        let unsettled: Vec<(Arc<str>, crate::symbols::SymbolInfoHandle)> = seed
            .iter_shared()
            .filter(|(_, symbol)| is_lazy_initializer(&symbol.ty))
            .map(|(name, symbol)| (name.clone(), symbol.clone()))
            .collect();
        for (name, symbol) in unsettled {
            let _ = seed.insert_shared(
                name,
                Arc::new(SymbolInfo {
                    ty: crate::checks::function::settle_lazy_read(symbol.ty.clone()),
                    kind: symbol.kind,
                    function_signature: symbol.function_signature.clone(),
                }),
            );
        }
        let declarations = ctx.type_declarations.clone();
        // The answer outlives the collection's shadow context, so the lazy
        // references it carries (`DecorateRouterRecord<TRoot, $Value>` for a
        // router's nested record) must intern into a store that does too.
        let table = with_source_exports_sharing_environment_store(|| {
            collect_exportable_value_symbols(
                std::slice::from_ref(&self.statement),
                &declarations,
                &seed,
                None,
                true,
                &ctx,
            )
        });
        table.get_own_shared(&self.name).map(|symbol| symbol.ty.clone())
    }
}

pub(crate) fn is_lazy_initializer(ty: &Type) -> bool {
    matches!(ty, Type::Reference(reference) if reference.id.contains(LAZY_INITIALIZER_ID_TAG))
}

fn lazy_initializer_reference(
    variable: &surge_ts_syntax::ParsedVariableDeclaration,
    origin: &CheckerContext,
) -> Type {
    let start = variable.name_span.map_or(0, |span| span.start);
    let id: Arc<str> = Arc::from(format!(
        "{}{LAZY_INITIALIZER_ID_TAG}{}\u{0}{start}",
        origin.file_name, variable.name
    ));
    let reference = surge_ts_types::TypeReference::new(
        id.clone(),
        format!("typeof {}", variable.name),
        Vec::new(),
        Arc::new(LazyInitializerValue {
            id,
            statement: ParsedStatement::VariableDeclaration(Box::new(variable.clone())),
            name: variable.name.clone(),
            file_name: Arc::from(origin.file_name.as_str()),
            environment: origin.declaration_environment(),
            creation_scope: origin.type_declaration_scope.clone(),
            memo: std::sync::OnceLock::new(),
        }),
    );
    Type::Reference(reference.rendered_structurally())
}

/// Runs `f` with source-file value collection interning its lazy references
/// into the caller's persistent declaration-environment store (see the store
/// comment in [`collect_exportable_value_symbols`]).
pub(crate) fn with_source_exports_sharing_environment_store<R>(f: impl FnOnce() -> R) -> R {
    let previous = SOURCE_EXPORTS_SHARE_ENVIRONMENT_STORE.with(|flag| flag.replace(true));
    let result = f();
    SOURCE_EXPORTS_SHARE_ENVIRONMENT_STORE.with(|flag| flag.set(previous));
    result
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
                if exportable_values.get_own(&namespace.name).is_none()
                    && crate::program::is_instantiated_namespace(namespace)
                {
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
                    let inner_prefix =
                        format!("{prefix}.{}", inner_namespace.member_name());
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
/// Each merging namespace's value members, with whether the name it merges into
/// is declared by a *function* — which decides whether the fallback object in
/// [`apply_merging_namespace_value_members`] stays callable.
fn merging_namespace_value_members(
    statements: &[ParsedStatement],
) -> Vec<(String, surge_ts_types::PropertyMap, bool)> {
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
    let mut function_names = surge_ts_types::fx::FxHashSet::default();
    for statement in statements {
        match peel_exported_statement(statement) {
            ParsedStatement::VariableDeclaration(variable) => {
                value_names.insert(variable.name.as_str());
            }
            ParsedStatement::FunctionDeclaration(function) => {
                value_names.insert(function.name.as_str());
                function_names.insert(function.name.as_str());
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

    let mut merged: Vec<(String, surge_ts_types::PropertyMap, bool)> = Vec::new();
    for statement in statements {
        let ParsedStatement::NamespaceDeclaration(namespace) = peel_exported_statement(statement)
        else {
            continue;
        };
        if !value_names.contains(namespace.name.as_str()) {
            continue;
        }
        let index = match merged.iter().position(|(name, ..)| name == &namespace.name) {
            Some(index) => index,
            None => {
                merged.push((
                    namespace.name.clone(),
                    surge_ts_types::PropertyMap::default(),
                    function_names.contains(namespace.name.as_str()),
                ));
                merged.len() - 1
            }
        };
        fill_namespace_value_properties(namespace, &mut merged[index].1);
    }
    merged
}

pub(crate) fn peel_exported_statement(statement: &ParsedStatement) -> &ParsedStatement {
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
    merging_namespaces: &[(String, surge_ts_types::PropertyMap, bool)],
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
        .any(|(name, ..)| name == &namespace.name)
}

/// Overlays each merging namespace's value members onto the value symbol the
/// walk produced, without displacing members the value already carries.
/// The value symbol is left alone when the namespace contributes no value
/// members (the `@types/node` shape: the namespace holds types only).
fn apply_merging_namespace_value_members(
    merging_namespaces: &[(String, surge_ts_types::PropertyMap, bool)],
    exportable_values: &mut SymbolTable,
) {
    for (name, members, declared_by_a_function) in merging_namespaces {
        let Some(symbol) = exportable_values.get_shared(name) else {
            // The value declaration bound nothing (an unsupported binding form);
            // fall back to the namespace object so the name stays a value.
            // A *function* merged with its namespace stays callable even when
            // the walk produced no value for it: a bare namespace object here
            // made `drizzle(client)` report TS2349 in every module that imports it.
            // The signature is deliberately permissive — the same shape an
            // unresolved callee already had — so nothing about the call is
            // newly checked, only that it is a call.
            let object = crate::metrics::alloc_object_type(members.clone(), None);
            let object = if *declared_by_a_function {
                object.with_call_signature(surge_ts_types::FunctionType::new(
                    vec![Type::Any],
                    Type::Any,
                    true,
                    0,
                ))
            } else {
                object
            };
            let _ = exportable_values.insert(
                name.clone(),
                SymbolInfo {
                    ty: Type::Object(object),
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
                crate::metrics::alloc_object_type(members.clone(), None)
                    .with_call_signature(function.clone()),
            ),
            Type::Unknown | Type::TypeParameter(_) => {
                Type::Object(crate::metrics::alloc_object_type(members.clone(), None))
            }
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

/// tsc merges a function declaration and a same-named namespace into one
/// symbol. Hoisted signature collection binds the function alone; this
/// overlays the namespace's value members onto it, and binds nothing a
/// declaration of the scope did not. (A class merges the same way, but surge
/// cannot yet tell its merged members from its `static` ones, which TS2576
/// depends on.)
pub(crate) fn apply_namespace_members_to_declarations(
    statements: &[ParsedStatement],
    symbols: &mut SymbolTable,
) {
    let merging: Vec<_> = merging_namespace_value_members(statements)
        .into_iter()
        .filter(|(name, _, declared_by_a_function)| {
            *declared_by_a_function
                && symbols
                    .get_own(name)
                    .is_some_and(|symbol| matches!(symbol.ty, Type::Function(_)))
        })
        .collect();
    apply_merging_namespace_value_members(&merging, symbols);
}

/// tsc binds a top-level `fn.x = value` as a declaration of `x` on `fn` when
/// `fn` is a function declaration or a `const` holding a function (an
/// *expando*): the value's type is `{ (…): R; x: typeof value }` for every
/// reader, in this module and in its importers (`Card.Header = Header`, then
/// `<Card.Header />`). Several writes of one name union their types.
pub(crate) fn apply_expando_members(
    statements: &[ParsedStatement],
    exportable_values: &mut SymbolTable,
    ctx: &mut CheckerContext,
) {
    let mut assignments = Vec::new();
    for statement in statements {
        match statement {
            ParsedStatement::MemberAssignment(assignment) => assignments.push(assignment.as_ref()),
            ParsedStatement::If(if_statement) => {
                collect_nested_member_assignments(&if_statement.then_body, &[], &mut assignments);
                collect_nested_member_assignments(&if_statement.else_body, &[], &mut assignments);
            }
            ParsedStatement::Block(body) => {
                collect_nested_member_assignments(body, &[], &mut assignments)
            }
            _ => {}
        }
    }
    for assignment in assignments {
        let surge_ts_syntax::ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } = &assignment.target
        else {
            continue;
        };
        let surge_ts_syntax::ParsedExpression::Identifier { name, .. } = object.as_ref() else {
            continue;
        };
        let Some(symbol) = exportable_values.get_own_shared(name) else {
            continue;
        };
        let (call_signature, mut properties) = match (&symbol.kind, &symbol.ty) {
            (SymbolKind::Function | SymbolKind::Const, Type::Function(function)) => {
                (function.clone(), surge_ts_types::PropertyMap::default())
            }
            (SymbolKind::Function | SymbolKind::Const, Type::Object(object)) => {
                let Some(signature) = object.call_signature() else {
                    continue;
                };
                (signature.clone(), (*object.properties).clone())
            }
            _ => continue,
        };
        let reported = ctx.diagnostics().len();
        let inferred =
            crate::infer::infer_expression(&assignment.value, exportable_values, ctx);
        ctx.truncate_diagnostics(reported);
        let crate::infer::InferredExpression::Known(value_type) = inferred else {
            continue;
        };
        if value_type.is_unknown() {
            continue;
        }
        let value_type = crate::checks::var::widen_implicit_variable_initializer_type(
            SymbolKind::Let,
            &assignment.value,
            &value_type,
            false,
        );
        let member_type = match properties.get(property_name.as_str()) {
            Some(existing) => surge_ts_types::union_type(vec![existing.ty.clone(), value_type]),
            None => value_type,
        };
        properties.insert(
            property_name.as_str().into(),
            surge_ts_types::ObjectProperty::required(member_type),
        );
        let kind = symbol.kind;
        let function_signature = symbol.function_signature.clone();
        let _ = exportable_values.insert(
            name.clone(),
            SymbolInfo {
                ty: Type::Object(
                    crate::metrics::alloc_object_type(properties, None)
                        .with_call_signature(call_signature.clone()),
                ),
                kind,
                function_signature,
            },
        );
    }
}

/// Member writes inside `if` and bare blocks: tsc binds an expando wherever it
/// sits in its container, not only at the top of it. A block that declares the
/// receiver's name itself (`const Y = …; Y.test = 42`) writes to its own
/// binding, which shadows the outer function.
fn collect_nested_member_assignments<'a>(
    body: &'a [surge_ts_syntax::ParsedFunctionBodyStatement],
    shadowed: &[&'a str],
    assignments: &mut Vec<&'a surge_ts_syntax::ParsedMemberAssignment>,
) {
    use surge_ts_syntax::ParsedFunctionBodyStatement as Statement;
    let mut shadowed = shadowed.to_vec();
    for statement in body {
        match statement {
            Statement::VariableDeclaration(variable) => shadowed.push(variable.name.as_str()),
            Statement::Function(function) => shadowed.push(function.name.as_str()),
            Statement::Class(class) => shadowed.push(class.name.as_str()),
            _ => {}
        }
    }
    for statement in body {
        match statement {
            Statement::MemberAssignment(assignment) => {
                let receiver = match &assignment.target {
                    surge_ts_syntax::ParsedExpression::PropertyAccess { object, .. } => {
                        match object.as_ref() {
                            surge_ts_syntax::ParsedExpression::Identifier { name, .. } => {
                                Some(name.as_str())
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if receiver.is_some_and(|name| !shadowed.contains(&name)) {
                    assignments.push(assignment.as_ref());
                }
            }
            Statement::If(if_statement) => {
                collect_nested_member_assignments(&if_statement.then_body, &shadowed, assignments);
                collect_nested_member_assignments(&if_statement.else_body, &shadowed, assignments);
            }
            Statement::Block(body) => {
                collect_nested_member_assignments(body, &shadowed, assignments)
            }
            _ => {}
        }
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

    let mut merged = crate::metrics::alloc_object_type(
        properties,
        object.string_index_type.as_ref().map(|ty| (**ty).clone()),
    );
    merged.alias_name = object.alias_name.clone();
    merged.alias_id = object.alias_id.clone();
    merged.construct_signature = object.construct_signature.clone();
    merged.call_signature = object.call_signature.clone();
    merged.is_intersection = object.is_intersection;
    merged.intersection_operands = object.intersection_operands.clone();
    merged.synthetic_open_index = object.synthetic_open_index;
    merged
}

pub(crate) fn collect_exportable_value_symbols(
    statements: &[ParsedStatement],
    local_type_declarations: &TypeDeclarationTable,
    local_symbols: &SymbolTable,
    imported_symbols: Option<&SymbolTable>,
    // Whether these statements are a *module* file's own top level. A module's
    // `declare const screen` is module-local and must bind ahead of a global of
    // the same name; the statements of an ambient `declare module` block inside
    // a script `.d.ts` (`@types/node`) are not a module file and keep the
    // global-wins behavior.
    module_file: bool,
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
    shadow_ctx.skip_annotated_function_bodies = true;
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
    } else if SOURCE_EXPORTS_SHARE_ENVIRONMENT_STORE.with(std::cell::Cell::get) {
        // Except for the value-export refinement rounds: the tables they
        // install are the ones consumers read, and a refined value is exactly
        // what peels its references. A recursive alias's back-edge there
        // (tRPC's `DecoratedProcedureUtilsRecord<TRoot, $Value>` behind
        // `useUtils().todo`) forced through a dead store turns the member
        // intersection into its `DecorateRouter` half alone, and every
        // procedure read off it into a TS2339.
        shadow_ctx.declaration_environment_store = ctx.declaration_environment_store.clone();
    }
    shadow_ctx.type_declaration_scope = ctx.type_declaration_scope.clone();
    shadow_ctx.ambient_global_type_declarations = ctx.ambient_global_type_declarations.clone();
    shadow_ctx.ambient_global_symbols = ctx
        .ambient_global_symbols
        .clone_with_reason(TypeCopyReason::ModuleExport);
    shadow_ctx.module_scope_by_file = ctx.module_scope_by_file.clone();
    shadow_ctx.module_local_values_by_file = ctx.module_local_values_by_file.clone();
    shadow_ctx.import_type_namespaces = ctx.import_type_namespaces.clone();
    shadow_ctx.import_type_globals = ctx.import_type_globals.clone();
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

    // Only the binding passes defer: the check phase infers every initializer
    // itself, with its imports already typed.
    let lazy_origin = (module_file && !library_file && !crate::program::in_check_phase())
        .then_some(ctx);
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
            module_file,
            lazy_origin,
        );
    }
    for hoisted in hoisted_nested_vars(statements) {
        if let ParsedStatement::VariableDeclaration(variable) = &hoisted
            && exportable_values.get_own_shared(&variable.name).is_none()
        {
            collect_exportable_value_symbols_from_statement(
                &hoisted,
                &mut exportable_values,
                &mut shadow_ctx,
                !library_file,
                module_file,
                lazy_origin,
            );
        }
    }
    apply_merging_namespace_value_members(&merging_namespaces, &mut exportable_values);
    apply_expando_members(statements, &mut exportable_values, &mut shadow_ctx);
    inherit_base_statics(statements, &mut exportable_values, imported_symbols);

    exportable_values
}

/// The `var`s nested in the top-level blocks, loops, `if`s, `switch`es and
/// `try`s of `statements`. tsc's binder declares a `var` in its function-like
/// container — the file or the namespace here — so it is in scope throughout
/// it, in a function declared ahead of it too. A `for…in` key is a `string`; a
/// `for…of` element is left unmodelled.
fn hoisted_nested_vars(statements: &[ParsedStatement]) -> Vec<ParsedStatement> {
    use surge_ts_syntax::ParsedFunctionBodyStatement as Statement;
    fn walk(body: &[Statement], out: &mut Vec<ParsedStatement>) {
        for statement in body {
            match statement {
                Statement::VariableDeclaration(variable)
                    if variable.kind == surge_ts_syntax::ParsedVariableKind::Var =>
                {
                    out.push(ParsedStatement::VariableDeclaration(variable.clone()));
                }
                Statement::Block(block) => walk(block, out),
                Statement::If(if_statement) => {
                    walk(&if_statement.then_body, out);
                    walk(&if_statement.else_body, out);
                }
                Statement::While(while_statement) => walk(&while_statement.body, out),
                Statement::ForOf(for_of_statement) => {
                    if for_of_statement.binding_kind == surge_ts_syntax::ParsedForBindingKind::Var
                        && let surge_ts_syntax::ParsedBindingName::Identifier { name, span } =
                            &for_of_statement.binding_name
                    {
                        out.push(ParsedStatement::VariableDeclaration(Box::new(
                            surge_ts_syntax::ParsedVariableDeclaration {
                                is_declare: false,
                                kind: surge_ts_syntax::ParsedVariableKind::Var,
                                from_binding_pattern: false,
                                has_definite_assertion: false,
                                array_pattern_span: None,
                                is_enum_object: false,
                                array_rest_start: None,
                                name: name.clone(),
                                name_span: *span,
                                declared_type: Some(if for_of_statement.keys_only {
                                    surge_ts_syntax::ParsedType::String
                                } else {
                                    surge_ts_syntax::ParsedType::Unknown
                                }),
                                initializer: None,
                                initializer_span: None,
                                declaration_list: None,
                            },
                        )));
                    }
                    walk(&for_of_statement.body, out)
                }
                Statement::Switch(switch_statement) => {
                    for case in &switch_statement.cases {
                        walk(&case.consequent, out);
                    }
                }
                Statement::Try(try_statement) => {
                    walk(&try_statement.block, out);
                    if let Some(handler) = &try_statement.handler {
                        walk(&handler.body, out);
                    }
                    walk(&try_statement.finalizer, out);
                }
                _ => {}
            }
        }
    }
    let mut hoisted = Vec::new();
    for statement in statements {
        match statement {
            ParsedStatement::Block(body) => walk(body, &mut hoisted),
            ParsedStatement::If(if_statement) => {
                walk(&if_statement.then_body, &mut hoisted);
                walk(&if_statement.else_body, &mut hoisted);
            }
            _ => {}
        }
    }
    hoisted
}

/// A derived class's static side starts from its base's, which the binding
/// pass could only read from the table as it stood *then*: a base's
/// namespace-merged members (`namespace EE { export const X }`) are applied
/// above, after both classes were bound, and a base bound by an import
/// (`class Stream extends EventEmitter` inside `declare module "stream"`) is
/// not in the table at all. Re-merge the base's statics now, from the merged
/// table or the import bindings. `prototype` stays the derived class's own.
pub(crate) fn inherit_base_statics(
    statements: &[ParsedStatement],
    exportable_values: &mut SymbolTable,
    imported_symbols: Option<&SymbolTable>,
) {
    for statement in statements {
        let ParsedStatement::ClassDeclaration(class) = peel_exported_statement(statement) else {
            continue;
        };
        let Some(base) = class.extends.first() else {
            continue;
        };
        let Some(base_type) = exportable_values
            .get(&base.name)
            .or_else(|| imported_symbols.and_then(|imported| imported.get(&base.name)))
            .map(|symbol| symbol.ty.peeled())
        else {
            continue;
        };
        let Some(merged) = statics_merged_into(&class.name, &base_type, exportable_values) else {
            continue;
        };
        let _ = exportable_values.insert(class.name.clone(), merged);
    }
}

/// `derived`'s value symbol with every static of `base_static` it does not
/// declare itself, or `None` when either side is not a static object or
/// nothing is missing. `prototype` is never inherited.
pub(crate) fn statics_merged_into(
    derived_name: &str,
    base_type: &Type,
    table: &SymbolTable,
) -> Option<SymbolInfo> {
    // Own-table only. A parent-traversing lookup finds an ambient global of the
    // same name when the module's own class has no value symbol — a generic
    // class models its value side as `Any` and contributes none — and then
    // merges the base's statics into *that*, publishing the DOM's
    // `MutationObserver` as the module's export (51 false `TS2554` across the
    // tanstack-query aggregate, where `new MutationObserver(client, options)`
    // met the DOM's one-parameter constructor).
    let derived = table.get_own(derived_name)?;
    statics_merged_into_symbol(derived, base_type)
}

/// `derived` with `base_type`'s statics merged in. A base surge models as
/// `any` (`@types/node`'s `namespace EventEmitter { export { internal as
/// EventEmitter } }` re-exports the module's own class as a member, which the
/// namespace lowering keeps permissive) contributes every name: the derived
/// static side is left open instead, so a consumer reading a static through it
/// gets `any` rather than a missing-export error.
pub(crate) fn statics_merged_into_symbol(
    derived: &SymbolInfo,
    base_type: &Type,
) -> Option<SymbolInfo> {
    let base_static = match base_type {
        Type::Object(object) => object,
        Type::Any => {
            let derived_peeled = derived.ty.peeled();
            let Type::Object(derived_static) = &derived_peeled else {
                return None;
            };
            if derived_static.synthetic_open_index {
                return None;
            }
            let mut opened = derived_static.clone().with_open_index_marker();
            opened.string_index_type = Some(std::sync::Arc::new(Type::Any));
            return Some(SymbolInfo {
                ty: Type::Object(opened),
                kind: derived.kind,
                function_signature: derived.function_signature.clone(),
            });
        }
        _ => return None,
    };
    {
        // A `.d.ts` value may be a lazy reference; the merged static side is
        // materialised, which is what every consumer reads anyway.
        let derived_peeled = derived.ty.peeled();
        let derived_static = match &derived_peeled {
            Type::Object(object) => object,
            _ => return None,
        };
        let missing: Vec<_> = base_static
            .properties
            .iter()
            .filter(|(name, _)| {
                name.as_ref() != "prototype"
                    && !derived_static.properties.contains_key(name.as_ref())
            })
            .map(|(name, property)| (name.clone(), property.clone()))
            .collect();
        if missing.is_empty() {
            return None;
        }
        let mut merged = derived_static.clone();
        let properties = std::sync::Arc::make_mut(&mut merged.properties);
        for (name, property) in missing {
            properties.insert(name, property);
        }
        Some(SymbolInfo {
            ty: Type::Object(merged),
            kind: derived.kind,
            function_signature: derived.function_signature.clone(),
        })
    }
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

/// Whether an annotation reads a module through `typeof import("spec")`. The
/// namespace behind it is registered when the file's imports are bound, which
/// runs after global collection, so such an annotation must be deferred.
pub(crate) fn annotation_contains_import_type_query(
    annotation: &surge_ts_syntax::ParsedType,
) -> bool {
    use surge_ts_syntax::ParsedType;
    match annotation {
        ParsedType::TypeOf(type_of) => type_of.import_specifier.is_some(),
        ParsedType::Array(element) | ParsedType::KeyOf(element) => {
            annotation_contains_import_type_query(element)
        }
        ParsedType::Tuple(elements)
        | ParsedType::Union(elements)
        | ParsedType::Intersection(elements) => {
            elements.iter().any(annotation_contains_import_type_query)
        }
        ParsedType::Named(named) => named
            .type_arguments
            .iter()
            .any(annotation_contains_import_type_query),
        ParsedType::IndexedAccess(indexed) => {
            annotation_contains_import_type_query(&indexed.object_type)
                || annotation_contains_import_type_query(&indexed.index_type)
        }
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
    module_file: bool,
    lazy_origin: Option<&CheckerContext>,
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
                // An ambient global declared as `typeof import("m")[name]` is
                // derived from this very export; letting it shadow the module's
                // own declaration would bind the export to the global and the
                // global to itself.
                let shadowed_by_import_type_global =
                    exportable_values.get_own_shared(&variable.name).is_none()
                        && ctx.is_import_type_global(&variable.name);
                // A module file's own `declare const` is module-local, so a
                // global of the same name never supersedes it: with the global
                // winning, `export declare const screen: Screen` in
                // `@testing-library/dom` published the DOM `window.screen`
                // instead, and every `screen.getByText(…)` in a consumer was a
                // false TS2339. The block statements of a script `.d.ts` keep
                // the old behavior — there a `declare const` *is* the global.
                let shadowed_by_global = module_file
                    && exportable_values.get_own_shared(&variable.name).is_none();
                if exportable_values.get_shared(&variable.name).is_none()
                    || shadowed_by_import_type_global
                    || shadowed_by_global
                {
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
            let lazy_reads_before = lazy_reads_before_check();
            let _ = check_variable_declaration_with_symbols(
                variable.as_ref().clone(),
                exportable_values,
                ctx,
                VariableCheckOptions {
                    report_duplicate_let_const: false,
                    check_initializer: check_initializers,
                },
            );
            let untyped = |symbol: &SymbolInfo| {
                if variable.declared_type.is_some() {
                    lazy_reads_before_check() != lazy_reads_before
                } else {
                    variable.initializer.is_some() && matches!(symbol.ty, Type::Unknown)
                }
            };

            if let Some(existing_symbol) = existing_symbol {
                exportable_values.insert_shared(variable.name.clone(), existing_symbol);
            } else if let Some(origin) = lazy_origin
                && check_initializers
                && !variable.from_binding_pattern
                && !variable.is_enum_object
                && let Some(symbol) = exportable_values.get_own_shared(&variable.name)
                && untyped(&symbol)
            {
                let _ = exportable_values.insert(
                    variable.name.clone(),
                    SymbolInfo {
                        ty: lazy_initializer_reference(variable, origin),
                        kind: symbol.kind,
                        function_signature: None,
                    },
                );
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
                    module_file,
                    lazy_origin,
                )
            }
        }
        ParsedStatement::NamespaceDeclaration(namespace) => {
            // A namespace declared twice in one file is one namespace: its
            // bodies merge, so the second block's members join the object the
            // first one produced rather than being dropped (or replacing it).
            let existing = exportable_values
                .get_own(&namespace.name)
                .map(|symbol| symbol.ty.clone());
            // A namespace of types alone has no value side (tsc's
            // `NamespaceModule`); a reference to it as a value is TS2708.
            let merges = crate::program::is_instantiated_namespace(namespace)
                && existing
                    .as_ref()
                    .is_none_or(|ty| matches!(ty, Type::Object(_)));
            if merges {
                let ty = namespace_value_object_type_resolved(namespace, ctx);
                let ty = match existing {
                    Some(previous) => merge_namespace_value_objects(&previous, &ty),
                    None => ty,
                };
                let _ = exportable_values.insert(
                    namespace.name.clone(),
                    SymbolInfo {
                        ty,
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
    // The second exception is a callback parameter (`ts.findConfigFile(dir,
    // (fileName) => …)`): the permissive member types every argument as
    // `any`, so the callback's own parameters become a false implicit-any.
    // The third exception is a `never` return (`util.assertNever(x)`): the
    // permissive member returns `any`, so the call no longer ends the flow and
    // the function it closes reports a missing return.
    let returns_never = matches!(return_type, Some(surge_ts_syntax::ParsedType::Never));
    let is_type_predicate = matches!(return_type, Some(surge_ts_syntax::ParsedType::Predicate(_)));
    let mut scan = SignatureNameScan::default();
    let mut takes_callback = false;
    for parameter_type in parameter_types {
        takes_callback |= parameter_type_is_callback(parameter_type);
        collect_signature_type_names(parameter_type, &mut scan);
    }
    if type_parameters.is_empty() && !is_type_predicate && !takes_callback && !returns_never {
        return false;
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

/// Whether a parameter is written as a callback — a function type, or an
/// optional/union one containing a function type. Only the written form is
/// consulted: resolving the annotation here would cost what publishing every
/// namespace member costs.
fn parameter_type_is_callback(parameter_type: &surge_ts_syntax::ParsedType) -> bool {
    match parameter_type {
        surge_ts_syntax::ParsedType::Function(_) => true,
        surge_ts_syntax::ParsedType::Union(members) => {
            members.iter().any(parameter_type_is_callback)
        }
        _ => false,
    }
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
    let ambient = [".d.ts", ".d.mts", ".d.cts"].iter().any(|extension| {
        ctx.file_name.len() >= extension.len()
            && ctx.file_name[ctx.file_name.len() - extension.len()..]
                .eq_ignore_ascii_case(extension)
    });
    namespace.statements.iter().any(|statement| {
        let (exported, inner) = match statement {
            ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
                ParsedExportDeclaration::Statement { declaration, .. } => {
                    (true, declaration.as_ref())
                }
                _ => return false,
            },
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
        ParsedType::Infer(infer) => scan.bound.push(infer.name.as_str()),
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
                // Overloads fold into one permissive signature, as file-level
                // declarations do (`React.useState<S>(initial)` next to
                // `useState<S = undefined>()`); the first declaration's parsed
                // signature stays the instantiation template.
                let (ty, function_signature) = match exportable_values.get_own(&key) {
                    Some(existing) => match &existing.ty {
                        Type::Function(existing_function)
                            if matches!(existing.kind, SymbolKind::Function) =>
                        {
                            (
                                Type::Function(
                                    crate::checks::function::merge_overload_group_signatures(
                                        existing_function,
                                        &function_type,
                                    ),
                                ),
                                crate::checks::function::mark_overloaded(
                                    existing.function_signature.clone(),
                                ),
                            )
                        }
                        _ => continue,
                    },
                    None => (Type::Function(function_type), Some(function_signature)),
                };
                let _ = exportable_values.insert(
                    key,
                    SymbolInfo {
                        ty,
                        kind: SymbolKind::Function,
                        function_signature,
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
                let inner_prefix =
                    format!("{prefix}.{}", inner_namespace.member_name());
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
/// Merges a later block of a namespace into the value object an earlier block
/// produced. Nested namespaces merge member by member at every depth, so
/// `namespace M { namespace N { … } }` written twice keeps both `N` bodies. A
/// namespace following the class or function it merges with adds its members
/// to that value (tsc's declaration merging keeps the constructor and call
/// signatures): a permissive class is `any`, which already has every member.
fn merge_namespace_value_objects(previous: &Type, current: &Type) -> Type {
    let (Type::Object(previous), Type::Object(current)) = (previous, current) else {
        return match (previous, current) {
            (Type::Any, _) => Type::Any,
            (Type::Function(function), Type::Object(current)) => Type::Object(
                crate::metrics::alloc_object_type(current.properties.as_ref().clone(), None)
                    .with_call_signature(function.clone()),
            ),
            _ => current.clone(),
        };
    };
    let mut properties = previous.properties.as_ref().clone();
    for (name, property) in current.properties.iter() {
        let merged = match properties.get(name) {
            Some(existing) => surge_ts_types::ObjectProperty {
                ty: merge_namespace_value_objects(&existing.ty, &property.ty),
                ..property.clone()
            },
            None => property.clone(),
        };
        properties.insert(name.clone(), merged);
    }
    Type::Object(crate::metrics::alloc_object_type(properties, None))
}

pub(crate) fn namespace_value_object_type(namespace: &ParsedNamespaceDeclaration) -> Type {
    let mut properties = surge_ts_types::PropertyMap::default();
    fill_namespace_value_properties(namespace, &mut properties);
    Type::Object(crate::metrics::alloc_object_type(properties, None))
}

/// [`namespace_value_object_type`] with the members' written annotations
/// resolved instead of left permissive.
///
/// A `declare namespace N { let X: Type<X> }` member is read through the
/// namespace *object* (`N.X`, and through an intersection that includes
/// `typeof N`), and a permissive `any` there is a false negative for every use
/// of it — `ast-types` declares its whole builder/named-type surface this way,
/// which is how `j.VariableDeclaration` reached tRPC's `upgrade` transforms as
/// `any`. Sibling names resolve under the namespace prefix, the same way a
/// member *signature* already does.
pub(crate) fn namespace_value_object_type_resolved(
    namespace: &ParsedNamespaceDeclaration,
    ctx: &mut CheckerContext,
) -> Type {
    let mut properties = surge_ts_types::PropertyMap::default();
    fill_namespace_value_properties(namespace, &mut properties);
    resolve_namespace_value_annotations(namespace, &namespace.name, &mut properties, ctx);
    Type::Object(crate::metrics::alloc_object_type(properties, None))
}

/// Replaces the permissive member types [`fill_namespace_value_properties`]
/// leaves for annotated `let`/`const`/`var` members with the resolved
/// annotation. Members without an annotation, and every other member kind, keep
/// what the permissive pass produced.
fn resolve_namespace_value_annotations(
    namespace: &ParsedNamespaceDeclaration,
    prefix: &str,
    properties: &mut surge_ts_types::PropertyMap,
    ctx: &mut CheckerContext,
) {
    for statement in &namespace.statements {
        match peel_exported_statement(statement) {
            ParsedStatement::VariableDeclaration(variable) => {
                let Some(annotation) = variable.declared_type.clone() else {
                    continue;
                };
                // A library member's annotation names imports the collection
                // scope does not hold (`Type` from "../types" in ast-types), so
                // it is deferred the way a library variable's annotation is and
                // maps on first read under the declaring file's environment.
                if ctx.lazy_library_value_annotations && defer_value_annotation(&annotation) {
                    let mut stack = ctx.namespace_member_prefix_stack.clone();
                    stack.push(prefix.to_string());
                    let ty = crate::infer::types::cache::make_lazy_value_annotation_reference_under(
                        ctx,
                        &format!("{prefix}.{}", variable.name),
                        variable.name_span.map_or(0, |span| span.start),
                        annotation,
                        Some(Arc::from(stack)),
                    );
                    properties.insert(
                        variable.name.as_str().into(),
                        surge_ts_types::ObjectProperty::required(ty),
                    );
                    continue;
                }
                ctx.namespace_member_resolution_depth += 1;
                ctx.namespace_member_prefix_stack.push(prefix.to_string());
                let resolved = crate::infer::map_parsed_type(annotation, ctx);
                ctx.namespace_member_prefix_stack.pop();
                ctx.namespace_member_resolution_depth -= 1;
                if resolved.is_unknown() {
                    continue;
                }
                properties.insert(
                    variable.name.as_str().into(),
                    surge_ts_types::ObjectProperty::required(resolved),
                );
            }
            ParsedStatement::NamespaceDeclaration(inner) => {
                let member_name = inner.member_name();
                let inner_prefix = format!("{prefix}.{member_name}");
                // Seeded with whatever a sibling block of the same namespace
                // already contributed, so the merge `fill_namespace_value_properties`
                // performed is not thrown away when the annotations resolve —
                // the call signature of a function it merged into included.
                let (mut inner_properties, call_signature) = match properties
                    .get(member_name)
                    .map(|property| &property.ty)
                {
                    // The class it merged into, whose `any` value stands.
                    Some(Type::Any) => continue,
                    Some(Type::Object(previous)) => (
                        previous.properties.as_ref().clone(),
                        previous.call_signature.clone(),
                    ),
                    _ => (surge_ts_types::PropertyMap::default(), None),
                };
                fill_namespace_value_properties(inner, &mut inner_properties);
                resolve_namespace_value_annotations(
                    inner,
                    &inner_prefix,
                    &mut inner_properties,
                    ctx,
                );
                let mut object = crate::metrics::alloc_object_type(inner_properties, None);
                object.call_signature = call_signature;
                properties.insert(
                    member_name.into(),
                    surge_ts_types::ObjectProperty::required(Type::Object(object)),
                );
            }
            _ => {}
        }
    }
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
            // `export import alias = N.M` makes the alias a member. Its entity's
            // members are reached through the rewritten references, so the
            // member itself stays permissive like the others here.
            ParsedStatement::ImportDeclaration(import)
                if !std::ptr::eq(inner, statement)
                    && let surge_ts_syntax::ParsedImportKind::EntityAlias { local_name, .. } =
                        &import.kind =>
            {
                properties.insert(local_name.as_str().into(), ObjectProperty::required(Type::Any));
            }
            ParsedStatement::FunctionDeclaration(function) => {
                // `(...args: any[]) => any`, spelled out rather than left as a
                // zero-parameter variadic: an argument lines up with the rest
                // parameter and is contextually typed `any`, where no parameter
                // at all leaves an arrow argument's own parameters untyped and
                // reports a false TS7006 on them.
                properties.insert(
                    function.name.as_str().into(),
                    ObjectProperty::required(Type::Function(FunctionType::new(
                        vec![Type::Array(Box::new(Type::Any))],
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
                let name: std::sync::Arc<str> = inner_namespace.member_name().into();
                let inner = namespace_value_object_type(inner_namespace);
                // A nested namespace written twice merges the same way a
                // top-level one does; without this the second block replaces
                // the first and its siblings' members go missing.
                let merged = match properties.get(&name) {
                    Some(previous) => merge_namespace_value_objects(&previous.ty, &inner),
                    None => inner,
                };
                properties.insert(name, ObjectProperty::required(merged));
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
