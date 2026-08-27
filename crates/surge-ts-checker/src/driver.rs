use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedInterfaceDeclaration,
    ParsedNamespaceDeclaration, ParsedStatement, ParsedType, ParsedTypeAliasDeclaration,
    parse_source,
};

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::{CheckerContext, DeclarationNamespace, DeclarationResolutionKey, FileKind};
use crate::default_lib::load_generated_default_lib_inputs;
use crate::infer::{report_duplicate_type_parameters, validate_local_type_declaration};
use crate::paths::canonicalize_if_exists_string;
use crate::program::collect_function_signatures_from_statements;
use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

pub fn check_source(source_text: &str, file_name: &str) -> Vec<Diagnostic> {
    check_source_with_options(
        source_text,
        file_name,
        crate::context::CheckerOptions::default(),
    )
}

pub fn check_source_with_options(
    source_text: &str,
    file_name: &str,
    options: crate::context::CheckerOptions,
) -> Vec<Diagnostic> {
    let parsed = parse_source(source_text, file_name);
    let file_name = parsed.file_name;
    let mut file_kinds = surge_ts_types::fx::FxHashMap::default();
    file_kinds.insert(file_name.clone(), classify_file_kind(&file_name));
    let mut ctx = CheckerContext::new(file_name.clone(), options, file_kinds);

    inject_generated_default_libs(&mut ctx);

    let mut merged_td = ctx.ambient_global_type_declarations.as_ref().clone();
    for (k, v) in ctx.type_declarations.iter() {
        let _ = merged_td.insert(k.clone(), v.clone());
    }
    ctx.type_declarations = merged_td;

    let mut merged_sym = ctx.ambient_global_symbols.clone();
    for (k, v) in ctx.symbols.iter() {
        let _ = merged_sym.insert(k.clone(), v.clone());
    }
    ctx.set_symbols(merged_sym);

    for message in parsed.parser_errors {
        let diagnostic = Diagnostic::surge_parser_error(message, file_name.clone());
        ctx.push(diagnostic);
    }

    collect_type_declarations(&parsed.statements, &mut ctx);
    collect_global_augmentations_from_statements(&parsed.statements, &mut ctx);
    sync_global_this_symbol(&mut ctx);
    let mut merged_sym = ctx.ambient_global_symbols.clone();
    for (k, v) in ctx.symbols.iter() {
        let _ = merged_sym.insert(k.clone(), v.clone());
    }
    ctx.set_symbols(merged_sym);

    let current_type_declarations = ctx.type_declarations.clone();
    let current_symbols = ctx.symbols.clone();
    let validation_symbols = crate::modules::collect_exportable_value_symbols(
        &parsed.statements,
        &current_type_declarations,
        &current_symbols,
        None,
        &mut ctx,
    );
    let saved_symbols = std::mem::replace(&mut ctx.symbols, validation_symbols);

    validate_local_type_declarations(&parsed.statements, &file_name, &mut ctx);
    validate_direct_utility_aliases(&parsed.statements, &mut ctx);
    let validation_symbols = std::mem::replace(&mut ctx.symbols, saved_symbols);

    ctx.module_value_fallback = Some(std::sync::Arc::new(validation_symbols));

    for statement in parsed.statements {
        check_statement(statement, &mut ctx);
    }
    ctx.module_value_fallback = None;

    ctx.finish()
}

fn inject_generated_default_libs(ctx: &mut CheckerContext) {
    let default_lib_inputs = load_generated_default_lib_inputs(ctx.options.no_lib, None);
    if default_lib_inputs.is_empty() {
        return;
    }

    let original_file_name = ctx.file_name.clone();
    let mut parser = surge_ts_syntax::ParserWorker::new();
    let parsed_files: Vec<crate::program::ParsedProgramFile> = default_lib_inputs
        .into_iter()
        .map(|input| {
            let parsed = parser.parse(&input.source_text, &input.file_name);
            crate::program::ParsedProgramFile {
                file_name: parsed.file_name,
                has_export_default: input.source_text.contains("export default"),
                contains_typeof: input.source_text.contains("typeof"),
                statements: parsed.statements,
                parser_errors: parsed.parser_errors,
                is_module: parsed.is_module,
                file_kind: FileKind::GeneratedDeclaration,
                module_reads: parsed.module_reads,
                suppressed_ranges: parsed.suppressed_ranges,
            }
        })
        .collect();

    crate::program::collect_ambient_globals(&parsed_files, ctx, None);
    ctx.set_file_name(original_file_name);
}

pub(crate) fn collect_type_declarations(statements: &[ParsedStatement], ctx: &mut CheckerContext) {
    for statement in statements {
        collect_type_declarations_from_statement(statement, ctx);
    }
}

/// Merge every `declare global { }` / `declare module "X" { global { } }`
/// augmentation *type* declaration across all files into the ambient global
/// table. A global interface split across files (e.g. `@types/node`'s
/// `BufferConstructor`, which gains `from` in `buffer.buffer.d.ts` and is named by
/// `var Buffer` in `buffer.d.ts`) must be fully assembled here before any value is
/// lowered against it during binding.
pub(crate) fn collect_global_augmentations(
    parsed_files: &[crate::program::ParsedProgramFile],
    ctx: &mut CheckerContext,
) {
    let mut block_tables = Vec::new();
    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());
        for_each_global_augmentation_block(
            &parsed_file.statements,
            ctx,
            |block_statements, ctx| {
                block_tables.push(collect_global_augmentation_block_types(
                    block_statements,
                    ctx,
                ));
            },
        );
    }
    crate::symbols::merge_shared_tables_into(
        std::sync::Arc::make_mut(&mut ctx.ambient_global_type_declarations),
        &block_tables,
    );
}

/// Lower `declare global` augmentation *values* against the caller's current type
/// environment. Called during binding (where the owning module's import scope is
/// active, so a `declare global { var x: ImportedType }` resolves) after
/// [`collect_global_augmentations`] has merged every augmentation type.
pub(crate) fn lower_global_augmentation_values_from_statements(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    for_each_global_augmentation_block(statements, ctx, lower_global_augmentation_values);
}

/// Single-file driver path: no cross-file split and no module import scope, so the
/// two phases run back-to-back over the one file's statements.
pub(crate) fn collect_global_augmentations_from_statements(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    let mut block_tables = Vec::new();
    for_each_global_augmentation_block(statements, ctx, |block_statements, ctx| {
        block_tables.push(collect_global_augmentation_block_types(
            block_statements,
            ctx,
        ));
    });
    crate::symbols::merge_shared_tables_into(
        std::sync::Arc::make_mut(&mut ctx.ambient_global_type_declarations),
        &block_tables,
    );
    for_each_global_augmentation_block(statements, ctx, lower_global_augmentation_values);
}

/// Whether any statement carries a `declare global` block (directly or nested
/// in a `declare module "x"`). Modules for which this is true publish global
/// values first-wins in the final analysis pass, which is order-sensitive and
/// must stay on the serial coordinator path.
pub(crate) fn has_global_augmentation_block(statements: &[ParsedStatement]) -> bool {
    statements.iter().any(|statement| {
        let ParsedStatement::DeclareModuleDeclaration(module) = statement else {
            return false;
        };
        module.module_specifier == "global"
            || module.statements.iter().any(|nested| {
                matches!(
                    nested,
                    ParsedStatement::DeclareModuleDeclaration(inner)
                        if inner.module_specifier == "global"
                )
            })
    })
}

fn for_each_global_augmentation_block(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
    mut visit: impl FnMut(&[ParsedStatement], &mut CheckerContext),
) {
    for statement in statements {
        let ParsedStatement::DeclareModuleDeclaration(module) = statement else {
            continue;
        };

        if module.module_specifier == "global" {
            visit(&module.statements, ctx);
            continue;
        }

        // `declare module "X" { global { ... } }` also augments the global scope
        // (e.g. `@types/node` declares `var Buffer` inside the "buffer" module).
        for nested in &module.statements {
            if let ParsedStatement::DeclareModuleDeclaration(inner) = nested {
                if inner.module_specifier == "global" {
                    visit(&inner.statements, ctx);
                }
            }
        }
    }
}

/// Collect one `declare global` block's type declarations into a fresh table, so
/// the result can later be merged into the ambient table by sharing payload
/// handles. The caller merges every block's table together in one pass
/// (see [`collect_global_augmentations`]).
fn collect_global_augmentation_block_types(
    block_statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) -> crate::symbols::TypeDeclarationTable {
    let saved_type_declarations = std::mem::replace(
        &mut ctx.type_declarations,
        crate::symbols::TypeDeclarationTable::new(),
    );
    let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
    ctx.type_declaration_scope = None;

    collect_type_declarations(block_statements, ctx);
    let block_table = std::mem::take(&mut ctx.type_declarations);

    ctx.type_declarations = saved_type_declarations;
    ctx.type_declaration_scope = saved_type_declaration_scope;

    block_table
}

fn lower_global_augmentation_values(
    block_statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    // Value types resolve against the caller's current type environment (during
    // binding: the module's local declarations plus its import scope) and the
    // ambient table. The block's own types were merged into the ambient table by
    // `merge_global_augmentation_types`, so a split global interface resolves to
    // its fully-merged form rather than this block's partial declaration.
    let saved_symbols = std::mem::take(&mut ctx.symbols);
    let mut current_symbols = crate::symbols::SymbolTable::new();
    let mut local_function_signatures = HashMap::new();
    collect_function_signatures_from_statements(
        block_statements,
        0,
        &mut current_symbols,
        &mut local_function_signatures,
        ctx,
    );
    ctx.symbols = current_symbols;

    for stmt in block_statements {
        let var = match stmt {
            ParsedStatement::VariableDeclaration(var) => Some(var),
            ParsedStatement::ExportDeclaration(export) => {
                if let surge_ts_syntax::ParsedExportDeclaration::Statement { declaration, .. } =
                    export.as_ref()
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

        // A `declare global { namespace awslambda { … } }` block publishes the
        // namespace as a global *value* too, so `awslambda.HttpResponseStream`
        // resolves. Same first-wins discipline the variable arm below uses.
        if let ParsedStatement::NamespaceDeclaration(namespace) = stmt
            && ctx.ambient_global_symbols.get(&namespace.name).is_none()
        {
            crate::program::record_augmentation_value_insertion();
            ctx.ambient_global_symbols.insert(
                namespace.name.clone(),
                crate::symbols::SymbolInfo {
                    ty: crate::modules::namespace_value_object_type(namespace),
                    kind: crate::symbols::SymbolKind::Const,
                    function_signature: None,
                },
            );
        }

        if let Some(var) = var {
            let ty = var
                .declared_type
                .as_ref()
                .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                .unwrap_or(surge_ts_types::Type::Unknown);
            if ctx.ambient_global_symbols.get(&var.name).is_none() {
                crate::program::record_augmentation_value_insertion();
                ctx.ambient_global_symbols.insert(
                    var.name.clone(),
                    crate::symbols::SymbolInfo {
                        ty,
                        kind: if matches!(var.kind, surge_ts_syntax::ParsedVariableKind::Const) {
                            crate::symbols::SymbolKind::Const
                        } else {
                            crate::symbols::SymbolKind::Let
                        },
                        function_signature: None,
                    },
                );
            }
        }
    }

    let mut ordered_function_signatures = local_function_signatures.into_iter().collect::<Vec<_>>();
    ordered_function_signatures
        .sort_by_key(|(location, _)| (location.file_index, location.statement_index));
    for (loc, fun_ty) in ordered_function_signatures {
        let name = match &block_statements[loc.statement_index] {
            ParsedStatement::FunctionDeclaration(f) => f.name.clone(),
            ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
                surge_ts_syntax::ParsedExportDeclaration::Default {
                    declaration: surge_ts_syntax::ParsedDefaultExportDeclaration::Function(f),
                    ..
                } => f.name.clone(),
                surge_ts_syntax::ParsedExportDeclaration::Statement { declaration, .. } => {
                    if let ParsedStatement::FunctionDeclaration(f) = declaration.as_ref() {
                        f.name.clone()
                    } else {
                        "unknown".to_string()
                    }
                }
                _ => "unknown".to_string(),
            },
            _ => "unknown".to_string(),
        };

        if ctx.ambient_global_symbols.get(&name).is_none() {
            crate::program::record_augmentation_value_insertion();
            ctx.ambient_global_symbols.insert(
                name,
                crate::symbols::SymbolInfo {
                    ty: surge_ts_types::Type::Function(fun_ty),
                    kind: crate::symbols::SymbolKind::Function,
                    function_signature: None,
                },
            );
        }
    }

    ctx.symbols = saved_symbols;
}

pub(crate) fn sync_global_this_symbol(ctx: &mut CheckerContext) {
    use surge_ts_types::PropertyMap;

    let mut properties = PropertyMap::default();
    for (name, symbol) in ctx.ambient_global_symbols.iter() {
        if name.as_ref() == "globalThis" {
            continue;
        }

        properties.insert(
            name.clone(),
            surge_ts_types::ObjectProperty::required(symbol.ty.clone()),
        );
    }

    ctx.ambient_global_symbols.insert(
        "globalThis".to_string(),
        crate::symbols::SymbolInfo {
            ty: surge_ts_types::Type::Object(crate::arena::alloc_object_type(properties, None)),
            kind: crate::symbols::SymbolKind::Const,
            function_signature: None,
        },
    );
}

pub(crate) fn validate_direct_utility_aliases(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        validate_direct_utility_aliases_from_statement(statement, ctx);
    }
}

pub(crate) fn validate_local_type_declarations(
    statements: &[ParsedStatement],
    file_name: &str,
    ctx: &mut CheckerContext,
) {
    let mut local_declarations = Vec::new();
    let mut seen = HashSet::new();

    collect_local_type_declarations_from_statements(
        statements,
        file_name,
        &mut seen,
        &mut local_declarations,
        ctx,
    );

    for declaration in local_declarations.into_iter().rev() {
        validate_local_type_declaration(&declaration, ctx);
    }
}

fn collect_local_type_declarations_from_statements(
    statements: &[ParsedStatement],
    file_name: &str,
    seen: &mut HashSet<DeclarationResolutionKey>,
    local_declarations: &mut Vec<TypeDeclarationInfo>,
    ctx: &CheckerContext,
) {
    for statement in statements {
        collect_local_type_declarations_from_statement(
            statement,
            file_name,
            seen,
            local_declarations,
            ctx,
        );
    }
}

fn collect_local_type_declarations_from_statement(
    statement: &ParsedStatement,
    file_name: &str,
    seen: &mut HashSet<DeclarationResolutionKey>,
    local_declarations: &mut Vec<TypeDeclarationInfo>,
    ctx: &CheckerContext,
) {
    match statement {
        ParsedStatement::TypeAliasDeclaration(alias) => {
            collect_named_local_type_declaration(
                &alias.name,
                file_name,
                LocalDeclarationKind::Alias,
                seen,
                local_declarations,
                ctx,
            );
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            collect_named_local_type_declaration(
                &interface.name,
                file_name,
                LocalDeclarationKind::Interface,
                seen,
                local_declarations,
                ctx,
            );
        }
        ParsedStatement::ClassDeclaration(class) => {
            collect_named_local_type_declaration(
                &class.name,
                file_name,
                LocalDeclarationKind::Interface,
                seen,
                local_declarations,
                ctx,
            );
        }
        ParsedStatement::ExportDeclaration(export) => {
            if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                collect_local_type_declarations_from_statement(
                    declaration.as_ref(),
                    file_name,
                    seen,
                    local_declarations,
                    ctx,
                )
            }
        }
        _ => {}
    }
}

#[derive(Clone, Copy)]
enum LocalDeclarationKind {
    Alias,
    Interface,
}

fn collect_named_local_type_declaration(
    name: &str,
    file_name: &str,
    kind: LocalDeclarationKind,
    seen: &mut HashSet<DeclarationResolutionKey>,
    local_declarations: &mut Vec<TypeDeclarationInfo>,
    ctx: &CheckerContext,
) {
    let canonical_file_name =
        crate::paths::canonicalize_if_exists_arc(std::path::Path::new(file_name));
    let key = DeclarationResolutionKey {
        file_name: canonical_file_name.clone(),
        name: std::sync::Arc::from(name),
        namespace: DeclarationNamespace::Type,
        fingerprint: 0,
    };
    if !seen.insert(key) {
        return;
    }

    let matches = |declaration: &TypeDeclarationInfo| match (kind, declaration) {
        (LocalDeclarationKind::Alias, TypeDeclarationInfo::Alias(info)) => {
            &*info.name == name
                && canonicalize_if_exists_string(std::path::Path::new(&*info.file_name))
                    == *canonical_file_name
        }
        (LocalDeclarationKind::Interface, TypeDeclarationInfo::Interface(info)) => {
            &*info.name == name
                && canonicalize_if_exists_string(std::path::Path::new(&*info.file_name))
                    == *canonical_file_name
        }
        _ => false,
    };

    // Local declarations are keyed by their own name, so the O(1) lookup covers
    // essentially every hit; the full scan only runs when the keyed entry was
    // shadowed (e.g. by an import alias) or absent.
    let declaration = ctx
        .type_declarations
        .get(name)
        .filter(|declaration| matches(declaration))
        .or_else(|| {
            ctx.type_declarations
                .iter()
                .map(|(_, declaration)| declaration)
                .find(|declaration| matches(declaration))
        });

    if let Some(declaration) = declaration {
        local_declarations.push(attach_current_type_scope_if_missing(
            declaration.clone(),
            ctx,
        ));
    }
}

fn attach_current_type_scope_if_missing(
    declaration: TypeDeclarationInfo,
    ctx: &CheckerContext,
) -> TypeDeclarationInfo {
    let current_scope = ctx.type_declaration_scope.clone().unwrap_or_else(|| {
        Arc::new(crate::symbols::TypeDeclarationScope::new(vec![Arc::new(
            ctx.type_declarations.clone(),
        )]))
    });

    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            alias.resolution_scope = Some(current_scope);
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            interface.resolution_scope = Some(current_scope);
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

fn collect_type_declarations_from_statement(statement: &ParsedStatement, ctx: &mut CheckerContext) {
    match statement {
        ParsedStatement::TypeAliasDeclaration(alias) => {
            collect_type_alias(alias, ctx);
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            collect_interface(interface, ctx);
        }
        ParsedStatement::ClassDeclaration(class) => {
            crate::program::collect_class(class, ctx);
        }
        ParsedStatement::ExportDeclaration(export) => match export.as_ref() {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                collect_type_declarations_from_statement(declaration.as_ref(), ctx)
            }
            ParsedExportDeclaration::Default {
                declaration: ParsedDefaultExportDeclaration::Class(class),
                ..
            } => {
                crate::program::collect_class(class, ctx);
            }
            _ => {}
        },
        ParsedStatement::NamespaceDeclaration(namespace) => {
            collect_namespace_type_declarations(namespace, ctx);
        }
        _ => {}
    }
}

/// Registers a namespace's interfaces and type aliases under qualified names
/// (`JSX.IntrinsicElements`, `JSX.Element`, ...) so the JSX checker can resolve
/// them. Members are not leaked into the unqualified scope. Nested namespaces are
/// flattened by their already-dotted names. Duplicate members are first-wins
/// (declaration merging), so no TS2300 is emitted here.
fn collect_namespace_type_declarations(
    namespace: &ParsedNamespaceDeclaration,
    ctx: &mut CheckerContext,
) {
    collect_namespace_type_declarations_prefixed(namespace, &namespace.name, ctx);
}

/// `prefix` is the fully-qualified dotted path to `namespace` (`React`, then
/// `React.JSX`, …). Each member is registered under the full path so a qualified
/// consumer reference like `React.JSX.IntrinsicElements` resolves; a nested member
/// is ALSO registered under the bare immediate-namespace key (`JSX.IntrinsicElements`)
/// so the JSX checker's literal lookup and the unqualified sibling references inside
/// the library's own bodies keep resolving. First-wins (declaration merging).
/// Default-on (opt-out `SURGE_NS_IFACE_MERGE=0`): fold a namespace's re-opened
/// interfaces into one declaration. Correct and worth **32 fewer false
/// positives on tRPC** (no new ones) — typescript.d.ts splits `Node`, `Type`,
/// `Symbol`, `Identifier`, `SourceFile`, `Signature` and four more into two
/// blocks each, and first-wins drops the block carrying their whole
/// service-method half (`ts.Type.getProperty`, `ts.Symbol.getName`,
/// `Node.getText`, `Identifier.text`).
///
/// The merged blocks are *mutually cyclic* (merged `ts.Node` gains methods
/// returning `SourceFile`, whose merged block returns `Node`), and a degraded
/// expansion is never interned, so before the check-phase degraded-peel pin on
/// `LazyInstantiation` every consumer peel re-expanded the cycle (+223% wall
/// on tRPC, member visits 3.3M -> 26.2M). With the pin the merge measures at
/// baseline wall (27.9s -> 8.1s on tRPC); the pin is what makes this default
/// affordable.
fn namespace_interface_merge_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("SURGE_NS_IFACE_MERGE").is_none_or(|value| value != "0")
    })
}

/// Merges from the parsed blocks in one shot rather than folding into the table
/// incrementally: the table is rebuilt for every consuming module, and folding an
/// already-merged result into itself re-appends its whole method set each pass
/// (measured as a further 4x on top of the cost above).
fn register_merged_namespace_interfaces(
    namespace: &ParsedNamespaceDeclaration,
    prefix: &str,
    bare_prefix: &str,
    ctx: &mut CheckerContext,
) {
    let mut blocks: std::collections::HashMap<
        &str,
        Vec<&surge_ts_syntax::ParsedInterfaceDeclaration>,
    > = std::collections::HashMap::new();
    let mut order: Vec<&str> = Vec::new();
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
        if let ParsedStatement::InterfaceDeclaration(interface) = inner {
            let entry = blocks.entry(interface.name.as_str()).or_default();
            if entry.is_empty() {
                order.push(interface.name.as_str());
            }
            entry.push(interface);
        }
    }

    for name in order {
        let interfaces = &blocks[name];
        if interfaces.len() < 2 {
            continue;
        }
        let file_name = ctx.file_name_arc();
        let build = |key: &str, interface: &surge_ts_syntax::ParsedInterfaceDeclaration| {
            InterfaceInfo::new(
                key.to_string(),
                file_name.clone(),
                interface.name_span,
                interface.type_parameters.clone(),
                interface.extends.clone(),
                interface.members.clone(),
                interface.string_index_type.clone(),
                interface.call_signature.clone(),
                interface.construct_signatures.clone(),
                None,
            )
        };
        let mut keys = vec![format!("{prefix}.{name}")];
        if prefix != bare_prefix {
            keys.push(format!("{bare_prefix}.{name}"));
        }
        for key in keys {
            let mut merged = build(&key, interfaces[0]);
            for interface in &interfaces[1..] {
                merged = crate::symbols::merge_interface_infos(&merged, &build(&key, interface));
            }
            ctx.type_declarations
                .upsert(key, TypeDeclarationInfo::Interface(merged));
        }
    }
}

fn collect_namespace_type_declarations_prefixed(
    namespace: &ParsedNamespaceDeclaration,
    prefix: &str,
    ctx: &mut CheckerContext,
) {
    let bare_prefix = namespace.name.as_str();
    if namespace_interface_merge_enabled() {
        register_merged_namespace_interfaces(namespace, prefix, bare_prefix, ctx);
    }
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
            ParsedStatement::InterfaceDeclaration(interface) => {
                let mut register = |key: String| {
                    let info = InterfaceInfo::new(
                        key.clone(),
                        ctx.file_name_arc(),
                        interface.name_span,
                        interface.type_parameters.clone(),
                        interface.extends.clone(),
                        interface.members.clone(),
                        interface.string_index_type.clone(),
                        interface.call_signature.clone(),
                        interface.construct_signatures.clone(),
                        None,
                    );
                    let _ = ctx
                        .type_declarations
                        .insert(key, TypeDeclarationInfo::Interface(info));
                };
                register(format!("{}.{}", prefix, interface.name));
                if prefix != bare_prefix {
                    register(format!("{}.{}", bare_prefix, interface.name));
                }
            }
            ParsedStatement::TypeAliasDeclaration(alias) => {
                let mut register = |key: String| {
                    let info = TypeAliasInfo::new(
                        key.clone(),
                        ctx.file_name_arc(),
                        alias.name_span,
                        alias.type_parameters.clone(),
                        alias.ty.clone(),
                        None,
                    );
                    let _ = ctx
                        .type_declarations
                        .insert(key, TypeDeclarationInfo::Alias(info));
                };
                register(format!("{}.{}", prefix, alias.name));
                if prefix != bare_prefix {
                    register(format!("{}.{}", bare_prefix, alias.name));
                }
            }
            // A class inside a namespace contributes an instance type under the
            // qualified key just as an interface does; without this a
            // `namespace NS { class C {} }` member is reachable as a value but
            // never as a type.
            ParsedStatement::ClassDeclaration(class) => {
                let mut register = |key: String| {
                    let mut info = crate::program::class_instance_interface_info(
                        class,
                        ctx.file_name_arc(),
                    );
                    info.declared_name = Some(info.name.clone());
                    info.name = key.as_str().into();
                    let _ = ctx
                        .type_declarations
                        .insert(key, TypeDeclarationInfo::Interface(info));
                };
                register(format!("{}.{}", prefix, class.name));
                if prefix != bare_prefix {
                    register(format!("{}.{}", bare_prefix, class.name));
                }
            }
            ParsedStatement::NamespaceDeclaration(inner_namespace) => {
                let inner_prefix = format!("{}.{}", prefix, inner_namespace.name);
                collect_namespace_type_declarations_prefixed(inner_namespace, &inner_prefix, ctx);
            }
            _ => {}
        }
    }
}

fn check_statement(statement: ParsedStatement, ctx: &mut CheckerContext) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            var::check_variable_declaration(*variable, ctx);
        }
        ParsedStatement::Assignment(assignment) => {
            assign::check_assignment(*assignment, ctx);
        }
        ParsedStatement::FunctionDeclaration(function) => {
            check_function::check_function_declaration(*function, ctx);
        }
        ParsedStatement::Call(call) => {
            call::check_call(*call, ctx);
        }
        ParsedStatement::Expression(expression) => {
            expr::check_expression_statement(*expression, ctx);
        }
        ParsedStatement::TypeAliasDeclaration(_) => {}
        ParsedStatement::InterfaceDeclaration(_) => {}
        ParsedStatement::ClassDeclaration(class) => {
            crate::program::check_class_declaration(&class, ctx);
        }
        ParsedStatement::ImportDeclaration(import) => {
            if crate::modules::is_external_specifier(&import.module_specifier) {
                let suppress_unresolved_diagnostic =
                    matches!(&import.kind, surge_ts_syntax::ParsedImportKind::SideEffect)
                        && is_runtime_js_only_module(&import.module_specifier, ctx);

                if !ctx.options.stub_external_modules && !suppress_unresolved_diagnostic {
                    let mut diagnostic = match &import.kind {
                        surge_ts_syntax::ParsedImportKind::SideEffect => {
                            Diagnostic::ts2882(&import.module_specifier, ctx.file_name.clone())
                        }
                        _ => Diagnostic::ts2307(&import.module_specifier, ctx.file_name.clone()),
                    };
                    if let Some(span) = import.module_specifier_span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }
                    ctx.push(diagnostic);
                }

                // A reported TS2307 means the program genuinely has no type
                // here, which is tsc's error type (`any`) — implicit-any and
                // argument checking downstream are real, not surge degradation.
                // Under `stubExternalModules` the diagnostic is suppressed on
                // purpose, so the sentinel stays and keeps the cascade quiet.
                let (binding_ty, binding_kind) = if ctx.options.stub_external_modules {
                    (
                        surge_ts_types::Type::Unknown,
                        crate::symbols::SymbolKind::Var,
                    )
                } else {
                    (
                        surge_ts_types::Type::Any,
                        crate::symbols::SymbolKind::ErrorImport,
                    )
                };
                // Stub the imports to avoid cascades in single-file mode
                match &import.kind {
                    surge_ts_syntax::ParsedImportKind::Named {
                        specifiers,
                        is_type_only,
                    } => {
                        for specifier in specifiers {
                            if *is_type_only {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo::new(
                                        specifier.local_name.to_string(),
                                        ctx.file_name_arc(),
                                        specifier.name_span,
                                        vec![],
                                        surge_ts_syntax::ParsedType::Unknown,
                                        None,
                                    ),
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                            } else {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo::new(
                                        specifier.local_name.to_string(),
                                        ctx.file_name_arc(),
                                        specifier.name_span,
                                        vec![],
                                        surge_ts_syntax::ParsedType::Unknown,
                                        None,
                                    ),
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                                let _ = ctx.symbols.insert(
                                    specifier.local_name.clone(),
                                    crate::symbols::SymbolInfo {
                                        ty: binding_ty.clone(),
                                        kind: binding_kind,
                                        function_signature: None,
                                    },
                                );
                            }
                        }
                    }
                    surge_ts_syntax::ParsedImportKind::DefaultAndNamed {
                        local_name,
                        name_span,
                        is_type_only,
                        specifiers,
                    } => {
                        if *is_type_only {
                            let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                crate::symbols::TypeAliasInfo::new(
                                    local_name.clone(),
                                    ctx.file_name_arc(),
                                    *name_span,
                                    vec![],
                                    surge_ts_syntax::ParsedType::Unknown,
                                    None,
                                ),
                            );
                            let _ = ctx
                                .type_declarations
                                .insert(local_name.clone(), declaration);
                        } else {
                            let _ = ctx.symbols.insert(
                                local_name.clone(),
                                crate::symbols::SymbolInfo {
                                    ty: binding_ty.clone(),
                                    kind: binding_kind,
                                    function_signature: None,
                                },
                            );
                        }

                        for specifier in specifiers {
                            if *is_type_only {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo::new(
                                        specifier.local_name.to_string(),
                                        ctx.file_name_arc(),
                                        specifier.name_span,
                                        vec![],
                                        surge_ts_syntax::ParsedType::Unknown,
                                        None,
                                    ),
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                            } else {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo::new(
                                        specifier.local_name.to_string(),
                                        ctx.file_name_arc(),
                                        specifier.name_span,
                                        vec![],
                                        surge_ts_syntax::ParsedType::Unknown,
                                        None,
                                    ),
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                                let _ = ctx.symbols.insert(
                                    specifier.local_name.clone(),
                                    crate::symbols::SymbolInfo {
                                        ty: binding_ty.clone(),
                                        kind: binding_kind,
                                        function_signature: None,
                                    },
                                );
                            }
                        }
                    }
                    surge_ts_syntax::ParsedImportKind::Default { local_name, .. } => {
                        let _ = ctx.symbols.insert(
                            local_name.clone(),
                            crate::symbols::SymbolInfo {
                                ty: binding_ty.clone(),
                                kind: binding_kind,
                                function_signature: None,
                            },
                        );
                    }
                    surge_ts_syntax::ParsedImportKind::Namespace {
                        local_name,
                        is_type_only,
                        ..
                    } => {
                        if *is_type_only {
                            let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                crate::symbols::TypeAliasInfo::new(
                                    local_name.clone(),
                                    ctx.file_name_arc(),
                                    None,
                                    vec![],
                                    surge_ts_syntax::ParsedType::Unknown,
                                    None,
                                ),
                            );
                            let _ = ctx
                                .type_declarations
                                .insert(local_name.clone(), declaration);
                        } else {
                            let _ = ctx.symbols.insert(
                                local_name.clone(),
                                crate::symbols::SymbolInfo {
                                    ty: binding_ty.clone(),
                                    kind: binding_kind,
                                    function_signature: None,
                                },
                            );
                        }
                    }
                    _ => {}
                }
            } else {
                let mut diagnostic =
                    Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = import.span.or(import.module_specifier_span) {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        }
        ParsedStatement::ExportDeclaration(export) => match *export {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                check_statement(*declaration, ctx)
            }
            ParsedExportDeclaration::Named {
                module_specifier: Some(specifier),
                span,
                module_specifier_span,
                ..
            } => {
                if crate::modules::is_external_specifier(&specifier) {
                    if !ctx.options.stub_external_modules {
                        let mut diagnostic = Diagnostic::ts2307(&specifier, ctx.file_name.clone());
                        if let Some(span) = module_specifier_span {
                            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                        }
                        ctx.push(diagnostic);
                    }
                } else {
                    let mut diagnostic =
                        Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                    if let Some(span) = span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }

                    ctx.push(diagnostic);
                }
            }
            ParsedExportDeclaration::Named { .. } => {}
            ParsedExportDeclaration::Namespace { .. } => {}
            ParsedExportDeclaration::Default { declaration, span } => match declaration {
                ParsedDefaultExportDeclaration::Function(function) => {
                    check_function::check_function_declaration(function, ctx);
                }
                ParsedDefaultExportDeclaration::Class(class) => {
                    crate::program::check_class_declaration(&class, ctx);
                }
                ParsedDefaultExportDeclaration::Expression(expression) => {
                    expr::check_expression_statement(expression, ctx);
                }
                ParsedDefaultExportDeclaration::Unsupported { .. } => {
                    let mut diagnostic =
                        Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                    if let Some(span) = span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }

                    ctx.push(diagnostic);
                }
            },
            ParsedExportDeclaration::All {
                module_specifier,
                span,
                module_specifier_span,
                ..
            } => {
                if crate::modules::is_external_specifier(&module_specifier) {
                    if !ctx.options.stub_external_modules {
                        let mut diagnostic =
                            Diagnostic::ts2307(&module_specifier, ctx.file_name.clone());
                        if let Some(span) = module_specifier_span {
                            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                        }
                        ctx.push(diagnostic);
                    }
                } else {
                    let mut diagnostic =
                        Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                    if let Some(span) = span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }

                    ctx.push(diagnostic);
                }
            }
            ParsedExportDeclaration::Empty { .. } => {}
            ParsedExportDeclaration::Equals { .. } => {}
            ParsedExportDeclaration::NamespaceExport { .. } => {}
            ParsedExportDeclaration::Unsupported { span } => {
                let mut diagnostic =
                    Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        },
        ParsedStatement::DeclareModuleDeclaration(_) => {}
        // Namespace members are bound during type-declaration collection; the
        // namespace itself produces no value-level checks here.
        ParsedStatement::NamespaceDeclaration(_) => {}
        ParsedStatement::UnsupportedDeclaration { span } => {
            let mut diag = surge_ts_diagnostics::Diagnostic::surge_unsupported_declaration(
                ctx.file_name.clone(),
            );
            if let Some(s) = span {
                diag = diag.with_span(crate::context::convert_span(s));
            }
            ctx.push(diag);
        }
    }
}

fn is_runtime_js_only_module(module_specifier: &str, ctx: &CheckerContext) -> bool {
    let Some(resolved_path) = ctx
        .options
        .resolved_module_for(&ctx.file_name, module_specifier)
    else {
        return false;
    };

    let lower = resolved_path.to_ascii_lowercase();
    lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs")
}

fn validate_direct_utility_aliases_from_statement(
    statement: &ParsedStatement,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::TypeAliasDeclaration(alias) => {
            validate_direct_utility_alias(alias, ctx);
        }
        ParsedStatement::ExportDeclaration(export) => {
            if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                validate_direct_utility_aliases_from_statement(declaration.as_ref(), ctx)
            }
        }
        _ => {}
    }
}

fn validate_direct_utility_alias(alias: &ParsedTypeAliasDeclaration, ctx: &mut CheckerContext) {
    if ctx.current_file_kind == crate::context::FileKind::GeneratedDeclaration {
        return;
    }

    let ParsedType::Named(named_type) = &alias.ty else {
        return;
    };

    if !matches!(
        named_type.name.as_str(),
        "Record" | "Partial" | "Pick" | "Omit"
    ) {
        return;
    }

    // The alias's own type parameters are in scope for its body. Seeding them as
    // placeholders keeps the utility's argument probe from reporting them as
    // unknown names, while the constraints-only push leaves the value scope empty
    // so the concrete-instantiation short-circuit still applies.
    let mut substitution = crate::infer::TypeParameterSubstitution::new();
    for type_parameter in &alias.type_parameters {
        substitution
            .insert_placeholder(type_parameter.name.clone(), surge_ts_types::Type::Unknown);
    }
    ctx.push_type_parameter_constraints_only(&alias.type_parameters);
    let _ = crate::infer::map_parsed_type_with_substitution(alias.ty.clone(), ctx, &substitution);
    ctx.pop_type_parameter_scope();
}

fn classify_file_kind(file_name: &str) -> FileKind {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts") {
        return FileKind::RootDeclaration;
    }

    FileKind::RootSource
}

pub(crate) fn collect_type_alias(alias: &ParsedTypeAliasDeclaration, ctx: &mut CheckerContext) {
    report_duplicate_type_parameters(&alias.type_parameters, ctx);

    let info = TypeAliasInfo::new(
        alias.name.clone(),
        ctx.file_name_arc(),
        alias.name_span,
        alias.type_parameters.clone(),
        alias.ty.clone(),
        None,
    );

    if ctx
        .type_declarations
        .insert(alias.name.clone(), TypeDeclarationInfo::Alias(info))
        .is_some()
    {
        let mut diagnostic = Diagnostic::ts2300(&alias.name, ctx.file_name.clone());

        if let Some(span) = alias.name_span {
            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
        }

        ctx.push(diagnostic);
    }
}

pub(crate) fn collect_interface(interface: &ParsedInterfaceDeclaration, ctx: &mut CheckerContext) {
    report_duplicate_type_parameters(&interface.type_parameters, ctx);

    let info = InterfaceInfo::new(
        interface.name.clone(),
        ctx.file_name_arc(),
        interface.name_span,
        interface.type_parameters.clone(),
        interface.extends.clone(),
        interface.members.clone(),
        interface.string_index_type.clone(),
        interface.call_signature.clone(),
        interface.construct_signatures.clone(),
        None,
    );

    enum Existing {
        None,
        NonInterface,
        Interface(crate::symbols::TypeDeclarationHandle),
    }

    // Classify the existing entry through an arena-backed handle so the common
    // declaration-merging case reads the previously accumulated interface in
    // place instead of deep-cloning its (often large) member list. The handle
    // keeps the backing arena alive and is decoupled from `ctx`, so the borrowed
    // interface stays valid while `ctx` is borrowed mutably below.
    let existing = match ctx.type_declarations.get_handle(&interface.name) {
        None => Existing::None,
        Some(handle) => match handle.get() {
            TypeDeclarationInfo::Interface(_) => Existing::Interface(handle),
            _ => Existing::NonInterface,
        },
    };

    match existing {
        Existing::None => {
            let _ = ctx
                .type_declarations
                .insert(interface.name.clone(), TypeDeclarationInfo::Interface(info));
        }
        Existing::Interface(handle) => {
            let TypeDeclarationInfo::Interface(existing) = handle.get() else {
                unreachable!("handle classified as interface above")
            };
            let incoming = filter_conflicting_interface_members(existing, info, ctx);
            let merged = crate::symbols::merge_interface_infos(existing, &incoming);
            ctx.type_declarations.upsert(
                interface.name.clone(),
                TypeDeclarationInfo::Interface(merged),
            );
        }
        Existing::NonInterface => {
            let mut diagnostic = Diagnostic::ts2300(&interface.name, ctx.file_name.clone());

            if let Some(span) = interface.name_span {
                diagnostic = diagnostic.with_span(crate::context::convert_span(span));
            }

            ctx.push(diagnostic);
        }
    }
}

/// Drop members of a later interface declaration whose property type conflicts
/// with the existing declaration and report TS2717 for each. The earlier
/// declaration's type wins (matching TypeScript), so assignability still checks
/// against the first-declared type. Same-named methods are kept as overloads.
fn filter_conflicting_interface_members(
    existing: &InterfaceInfo,
    mut incoming: InterfaceInfo,
    ctx: &mut CheckerContext,
) -> InterfaceInfo {
    // First declaration of each name wins, matching the `find` this replaces.
    let mut existing_by_name = HashMap::new();
    for member in &existing.body.members {
        existing_by_name
            .entry(member.name.as_str())
            .or_insert(member);
    }

    std::sync::Arc::make_mut(&mut incoming.body)
        .members
        .retain(|member| {
            let Some(previous) = existing_by_name.get(member.name.as_str()).copied() else {
                return true;
            };

            let is_method = |ty: &ParsedType| matches!(ty, ParsedType::Function(_));
            if is_method(&previous.ty) || is_method(&member.ty) || previous.ty == member.ty {
                return true;
            }

            if let (Some(expected), Some(actual)) = (
                parsed_type_display(&previous.ty),
                parsed_type_display(&member.ty),
            ) {
                let mut diagnostic =
                    Diagnostic::ts2717(&member.name, expected, actual, ctx.file_name.clone());
                if let Some(span) = member.name_span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }
                ctx.push(diagnostic);
            }

            false
        });

    incoming
}

/// A best-effort TypeScript-style rendering of a parsed type for conflict
/// messages. Returns `None` for shapes whose display would not match the
/// compiler exactly; the caller then keeps the first declaration silently.
pub(crate) fn parsed_type_display(ty: &ParsedType) -> Option<String> {
    let rendered = match ty {
        ParsedType::String => "string".to_string(),
        ParsedType::Number => "number".to_string(),
        ParsedType::Boolean => "boolean".to_string(),
        ParsedType::Undefined => "undefined".to_string(),
        ParsedType::Void => "void".to_string(),
        ParsedType::Any => "any".to_string(),
        ParsedType::Unknown | ParsedType::UnknownKeyword => "unknown".to_string(),
        ParsedType::Never => "never".to_string(),
        ParsedType::StringLiteral(value) => format!("\"{value}\""),
        ParsedType::NumberLiteral(value) => value.clone(),
        ParsedType::BooleanLiteral(value) => value.to_string(),
        ParsedType::Named(named) if named.type_arguments.is_empty() => named.name.clone(),
        _ => return None,
    };
    Some(rendered)
}
