//! Ambient global and ambient-module (`declare module "..."`) collection passes.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use surge_ts_syntax::ParsedStatement;

use super::*;

use crate::context::{CheckerContext, FileKind};
use crate::driver::collect_type_declarations;
use crate::modules::{ModuleExportTable, build_module_export_table};

/// See the gate comment at the ambient block-import binding phase in
/// [`collect_ambient_modules`].
fn ambient_block_imports_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SURGE_AMBIENT_BLOCK_IMPORTS").is_some())
}
use crate::symbols::{SymbolTable, TypeDeclarationScope, TypeDeclarationTable};

#[derive(Debug, Clone)]
pub(crate) struct AmbientModuleEntry {
    module_specifier: String,
    file: ParsedProgramFile,
    raw_export_table: ModuleExportTable,
    /// The block's own type declarations, retained so the per-file ambient
    /// scope can be published after every block is registered.
    block_scope: Arc<TypeDeclarationScope>,
}

/// Collects the program's UMD global names: every `export as namespace X` in a
/// file that is itself a module. `X` is then reachable from script files but is
/// TS2686 from a module, which is what [`CheckerContext::umd_global_names`]
/// drives. A `export as namespace` in a script file declares nothing extra —
/// the file's declarations are already global — so only modules contribute.
pub(crate) fn collect_umd_global_names(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
) {
    let mut names: surge_ts_types::fx::FxHashSet<Arc<str>> = Default::default();

    for parsed_file in parsed_files {
        if !parsed_file.is_module {
            continue;
        }

        for statement in &parsed_file.statements {
            if let ParsedStatement::ExportDeclaration(export) = statement
                && let surge_ts_syntax::ParsedExportDeclaration::NamespaceExport {
                    exported_name,
                    ..
                } = export.as_ref()
            {
                names.insert(Arc::from(exported_name.as_str()));
            }
        }
    }

    ctx.umd_global_names = Arc::new(names);
}

/// Every name a file binds at module scope, by syntax alone. A UMD global is
/// shadowed by any such declaration whether or not surge managed to bind it, so
/// this must not be derived from the analysis tables.
pub(crate) fn module_scope_declared_names(statements: &[ParsedStatement]) -> HashSet<&str> {
    fn collect<'a>(statements: &'a [ParsedStatement], names: &mut HashSet<&'a str>) {
        for statement in statements {
            match statement {
                ParsedStatement::VariableDeclaration(variable) => {
                    names.insert(variable.name.as_str());
                }
                ParsedStatement::FunctionDeclaration(function) => {
                    names.insert(function.name.as_str());
                }
                ParsedStatement::ClassDeclaration(class) => {
                    names.insert(class.name.as_str());
                }
                ParsedStatement::InterfaceDeclaration(interface) => {
                    names.insert(interface.name.as_str());
                }
                ParsedStatement::TypeAliasDeclaration(alias) => {
                    names.insert(alias.name.as_str());
                }
                ParsedStatement::NamespaceDeclaration(namespace) => {
                    names.insert(namespace.name.as_str());
                }
                ParsedStatement::ExportDeclaration(export) => {
                    if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref()
                    {
                        collect(std::slice::from_ref(declaration.as_ref()), names);
                    }
                }
                _ => {}
            }
        }
    }

    let mut names = HashSet::new();
    collect(statements, &mut names);
    names.extend(import_bound_names(statements));
    names
}

/// Every local name a file's imports bind, type-only imports included. A
/// type-only binding still shadows a same-named global — tsc reports using it
/// as a value as TS1361, not as a UMD-global reference — and an import whose
/// module fails to resolve binds the name just as much, so this reads the
/// syntax rather than the resolved binding tables.
pub(crate) fn import_bound_names(statements: &[ParsedStatement]) -> HashSet<&str> {
    let mut names = HashSet::new();

    for statement in statements {
        let ParsedStatement::ImportDeclaration(import) = statement else {
            continue;
        };

        match &import.kind {
            surge_ts_syntax::ParsedImportKind::Named { specifiers, .. } => {
                names.extend(
                    specifiers
                        .iter()
                        .map(|specifier| specifier.local_name.as_str()),
                );
            }
            surge_ts_syntax::ParsedImportKind::DefaultAndNamed {
                local_name,
                specifiers,
                ..
            } => {
                names.insert(local_name.as_str());
                names.extend(
                    specifiers
                        .iter()
                        .map(|specifier| specifier.local_name.as_str()),
                );
            }
            surge_ts_syntax::ParsedImportKind::Default { local_name, .. }
            | surge_ts_syntax::ParsedImportKind::Namespace { local_name, .. }
            | surge_ts_syntax::ParsedImportKind::Equals { local_name, .. }
            | surge_ts_syntax::ParsedImportKind::TypeOnlyDefault { local_name, .. } => {
                names.insert(local_name.as_str());
            }
            surge_ts_syntax::ParsedImportKind::SideEffect
            | surge_ts_syntax::ParsedImportKind::Unsupported => {}
        }
    }

    names
}

pub(crate) fn collect_ambient_globals(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) {
    // Phase 1: collect and merge every ambient global *type* declaration across
    // all declaration files before any value symbol is lowered. The default lib
    // graph splits a single global interface across files (e.g. `ArrayConstructor`
    // gains `isArray` in lib.es5 and `from`/`of` in lib.es2015.core), and a
    // `declare var Array: ArrayConstructor` would otherwise freeze the variable's
    // type against whatever members were merged when its own file was processed,
    // dropping members contributed by files processed later.
    for parsed_file in parsed_files {
        if !is_ambient_global_declaration_file(parsed_file, ctx) {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;

        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        let collect_start = Instant::now();
        collect_type_declarations(&parsed_file.statements, ctx);
        let ambient_td = std::mem::take(&mut ctx.type_declarations);
        let lowered_type_declarations = ambient_td.len() as u64;
        let collect_duration = collect_start.elapsed();
        record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
            metrics.collect_type_declarations_passes += 1;
            metrics.lowered_type_declarations += lowered_type_declarations;
            metrics.collect_type_declarations_duration += collect_duration;
        });
        record_program_timing(timings, |timings| {
            timings.dependency_declaration_collection += collect_duration;
            timings.dependency_declaration_lower_time += collect_duration;
        });

        // Declaration merging across global declaration files: the same
        // interface (a default lib's `Window`, or a project's split global
        // `interface Env`) contributes members from every declaration rather
        // than being dropped first-wins.
        crate::symbols::merge_shared_table_into(
            Arc::make_mut(&mut ctx.ambient_global_type_declarations),
            &ambient_td,
        );

        ctx.type_declarations = saved_type_declarations;
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }

    // Phase 2: lower ambient value symbols (functions, `declare var`s, and
    // `declare class` constructors) against the now fully-merged type table, so
    // a variable typed by a split global interface sees every member.
    for parsed_file in parsed_files {
        if !is_ambient_global_declaration_file(parsed_file, ctx) {
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;
        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());

        let mut local_function_signatures = HashMap::new();
        let mut current_symbols = std::mem::take(&mut ctx.symbols);
        collect_function_signatures_from_statements(
            &parsed_file.statements,
            0,
            &mut current_symbols,
            &mut local_function_signatures,
            ctx,
        );
        ctx.symbols = current_symbols;

        for stmt in &parsed_file.statements {
            let var = match stmt {
                ParsedStatement::VariableDeclaration(var) => Some(var),
                ParsedStatement::ExportDeclaration(export) => {
                    if let surge_ts_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    } = export.as_ref()
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

            if let Some(var) = var {
                let ty = var
                    .declared_type
                    .as_ref()
                    .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                    .unwrap_or(surge_ts_types::Type::Unknown);
                if ctx.ambient_global_symbols.get(&var.name).is_none() {
                    ctx.ambient_global_symbols.insert(
                        var.name.clone(),
                        crate::symbols::SymbolInfo {
                            ty,
                            kind: if matches!(var.kind, surge_ts_syntax::ParsedVariableKind::Const)
                            {
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

        let mut ordered_function_signatures =
            local_function_signatures.into_iter().collect::<Vec<_>>();
        ordered_function_signatures
            .sort_by_key(|(location, _)| (location.file_index, location.statement_index));
        for (loc, fun_ty) in ordered_function_signatures {
            let name = match &parsed_file.statements[loc.statement_index] {
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

        // `declare class` contributes a global constructor/static value. The
        // instance interface is already in `ambient_global_type_declarations`
        // above, so the value's construct signature and member types resolve.
        for stmt in &parsed_file.statements {
            let class = match stmt {
                ParsedStatement::ClassDeclaration(class) => Some(class),
                ParsedStatement::ExportDeclaration(export) => {
                    if let surge_ts_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    } = export.as_ref()
                    {
                        if let ParsedStatement::ClassDeclaration(class) = declaration.as_ref() {
                            Some(class)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(class) = class {
                if ctx.ambient_global_symbols.get(&class.name).is_none() {
                    let symbol = super::build_class_value_symbol(class, ctx);
                    ctx.ambient_global_symbols
                        .insert(class.name.clone(), symbol);
                }
            }
        }

        ctx.type_declarations = saved_type_declarations;
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }

    lower_ambient_namespace_values(parsed_files, ctx);
}

/// `declare namespace X { ... }` contributes a global value object whose members
/// are the namespace's declarations. roblox-ts's lib uses this heavily (`math`,
/// `task`, `utf8`, `buffer`, `vector`, `os`); user code accesses them as values
/// (`math.floor`, `task.wait`). Without this the name resolves only as a type
/// (TS2693) or not at all (TS2304).
///
/// Namespaces merge across blocks and files exactly like interfaces — roblox-ts's
/// `math` is split across declarations — so members are accumulated across every
/// ambient block of the same name before a single value symbol is inserted. A
/// real value symbol (`declare const`/`function`/`class` of the same name) takes
/// precedence and is left untouched.
fn lower_ambient_namespace_values(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    use surge_ts_types::PropertyMap;

    let mut merged: HashMap<String, PropertyMap> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for parsed_file in parsed_files {
        if !is_ambient_global_declaration_file(parsed_file, ctx) {
            continue;
        }

        for stmt in &parsed_file.statements {
            let namespace = match stmt {
                ParsedStatement::NamespaceDeclaration(namespace) => Some(namespace),
                ParsedStatement::ExportDeclaration(export) => {
                    if let surge_ts_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    } = export.as_ref()
                    {
                        if let ParsedStatement::NamespaceDeclaration(namespace) =
                            declaration.as_ref()
                        {
                            Some(namespace)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(namespace) = namespace {
                let entry = merged.entry(namespace.name.clone()).or_insert_with(|| {
                    order.push(namespace.name.clone());
                    PropertyMap::default()
                });
                crate::modules::fill_namespace_value_properties(namespace, entry);
            }
        }
    }

    let global_augmentation_value_names = global_augmentation_value_names(parsed_files);

    for name in order {
        if ctx.ambient_global_symbols.get(&name).is_some() {
            continue;
        }
        let properties = merged.remove(&name).unwrap_or_default();
        // A namespace with no value members contributes nothing but an empty
        // object, and this pass runs before `declare global` blocks are lowered
        // (`collect_global_augmentations`) — claiming the name here would freeze
        // it at `{}` and turn every member access into TS2339. bun-types is
        // exactly this shape: `declare namespace Bun { type … }` in one file,
        // `declare global { var Bun: typeof import("bun") }` in another.
        if properties.is_empty() && global_augmentation_value_names.contains(&name) {
            continue;
        }
        ctx.ambient_global_symbols.insert(
            name,
            crate::symbols::SymbolInfo {
                ty: surge_ts_types::Type::Object(crate::arena::alloc_object_type(properties, None)),
                kind: crate::symbols::SymbolKind::Const,
                function_signature: None,
            },
        );
    }
}

/// Names bound as a global *value* by a `declare global { … }` block anywhere in
/// the program (including the `declare module "x" { global { … } }` nesting
/// `@types/node` uses). Those blocks are lowered after this pass, so their
/// values would otherwise lose the first-wins race against a same-named
/// ambient namespace.
fn global_augmentation_value_names(parsed_files: &[ParsedProgramFile]) -> HashSet<String> {
    fn collect_block_value_names(
        block_statements: &[ParsedStatement],
        names: &mut HashSet<String>,
    ) {
        for statement in block_statements {
            let inner = match statement {
                ParsedStatement::ExportDeclaration(export) => {
                    if let surge_ts_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    } = export.as_ref()
                    {
                        declaration.as_ref()
                    } else {
                        statement
                    }
                }
                other => other,
            };
            match inner {
                ParsedStatement::VariableDeclaration(var) => {
                    names.insert(var.name.clone());
                }
                ParsedStatement::FunctionDeclaration(function) => {
                    names.insert(function.name.clone());
                }
                ParsedStatement::ClassDeclaration(class) => {
                    names.insert(class.name.clone());
                }
                _ => {}
            }
        }
    }

    let mut names = HashSet::new();
    for parsed_file in parsed_files {
        for statement in &parsed_file.statements {
            let ParsedStatement::DeclareModuleDeclaration(module) = statement else {
                continue;
            };
            if module.module_specifier == "global" {
                collect_block_value_names(&module.statements, &mut names);
                continue;
            }
            for nested in &module.statements {
                if let ParsedStatement::DeclareModuleDeclaration(inner) = nested
                    && inner.module_specifier == "global"
                {
                    collect_block_value_names(&inner.statements, &mut names);
                }
            }
        }
    }
    names
}

/// Whether a parsed file contributes to the ambient global scope. Declaration
/// files do, except dependency declarations that are not part of a configured
/// `@types/*` package (those reach the program only through module resolution).
fn is_ambient_global_declaration_file(
    parsed_file: &ParsedProgramFile,
    ctx: &CheckerContext,
) -> bool {
    if !parsed_file.file_kind.is_declaration() {
        return false;
    }

    if parsed_file.file_kind == FileKind::DependencyDeclaration
        && !is_configured_types_global_file(&parsed_file.file_name, &ctx.options.types)
    {
        return false;
    }

    true
}

/// Whether `file_name` belongs to one of the configured `compilerOptions.types`
/// packages. Two layouts contribute global declarations: DefinitelyTyped stubs
/// under `node_modules/@types/<mangled>` (scoped names map like TypeScript:
/// `@scope/pkg` -> `scope__pkg`), and packages that ship their own ambient
/// declarations directly under `node_modules/<name>` (e.g. roblox-ts's
/// `@rbxts/types` / `@rbxts/compiler-types`, which replace the default lib). The
/// `types` list passed here already includes packages discovered through
/// `/// <reference types="..." />` closure, so a referenced package is covered
/// even when only its referrer is named explicitly.
pub(crate) fn is_configured_types_global_file(file_name: &str, types: &[String]) -> bool {
    let normalized = file_name.replace('\\', "/");
    types.iter().any(|type_name| {
        if type_name == "*" {
            return false;
        }
        let mangled = mangle_types_package_name(type_name);
        let at_types = format!("/@types/{mangled}/");
        let direct = format!("/node_modules/{type_name}/");
        normalized.contains(&at_types) || normalized.contains(&direct)
    })
}

fn mangle_types_package_name(type_name: &str) -> String {
    type_name
        .strip_prefix('@')
        .map(|name| name.replace('/', "__"))
        .unwrap_or_else(|| type_name.to_string())
}

pub(crate) fn collect_ambient_modules(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) {
    let ambient_binding_start = Instant::now();
    let mut ambient_module_entries = Vec::<AmbientModuleEntry>::new();
    let mut ambient_module_indexes = HashMap::<String, usize>::new();

    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;
        for statement in &parsed_file.statements {
            let ParsedStatement::DeclareModuleDeclaration(module) = statement else {
                continue;
            };

            if module.module_specifier == "global" {
                continue;
            }

            let saved_type_declarations =
                std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
            let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());

            // `declare module "buffer" { global { var Buffer: … } export { Buffer }; }`
            // resolves its own `export { … }` against the values its nested
            // `global` block declares, so those join the block's local scope.
            // Only the values: the block's *types* are merged program-wide by
            // `collect_global_augmentations`, and a module-local copy would
            // shadow that merge with one file's half of a split interface
            // (`BufferConstructor` spans buffer.d.ts and buffer.buffer.d.ts).
            // Nothing here reaches the export table on its own — only an
            // explicit `export` clause pulls a name out.
            let nested_global_statements: Vec<ParsedStatement> = module
                .statements
                .iter()
                .filter_map(|statement| match statement {
                    ParsedStatement::DeclareModuleDeclaration(nested)
                        if nested.module_specifier == "global" =>
                    {
                        Some(nested.statements.iter().cloned().map(|statement| {
                            // A `var` inside `declare global` is ambient by
                            // context, not by an explicit `declare` keyword.
                            match statement {
                                ParsedStatement::VariableDeclaration(mut declaration) => {
                                    declaration.is_declare = true;
                                    ParsedStatement::VariableDeclaration(declaration)
                                }
                                other => other,
                            }
                        }))
                    }
                    _ => None,
                })
                .flatten()
                .collect();

            let collect_start = Instant::now();
            collect_type_declarations(&module.statements, ctx);
            record_type_declaration_table_clone(
                timings,
                ctx.type_declarations.len(),
                TableCloneKind::General,
            );
            let current_type_declarations_scope =
                Arc::new(TypeDeclarationScope::new(vec![Arc::new(
                    ctx.type_declarations.clone(),
                )]));
            ctx.type_declaration_scope = Some(current_type_declarations_scope.clone());
            let mut local_function_signatures = HashMap::new();
            let mut current_symbols = std::mem::take(&mut ctx.symbols);
            collect_function_signatures_from_statements(
                &module.statements,
                0,
                &mut current_symbols,
                &mut local_function_signatures,
                ctx,
            );
            ctx.symbols = current_symbols;

            for stmt in module.statements.iter().chain(&nested_global_statements) {
                match stmt {
                    ParsedStatement::VariableDeclaration(var) => {
                        if var.is_declare && ctx.symbols.get(&var.name).is_none() {
                            let ty = var
                                .declared_type
                                .as_ref()
                                .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                                .unwrap_or(surge_ts_types::Type::Unknown);
                            ctx.symbols.insert(
                                var.name.clone(),
                                crate::symbols::SymbolInfo {
                                    kind: if matches!(
                                        var.kind,
                                        surge_ts_syntax::ParsedVariableKind::Const
                                    ) {
                                        crate::symbols::SymbolKind::Const
                                    } else {
                                        crate::symbols::SymbolKind::Let
                                    },
                                    ty,
                                    function_signature: None,
                                },
                            );
                        }
                    }
                    ParsedStatement::ExportDeclaration(export) => {
                        if let surge_ts_syntax::ParsedExportDeclaration::Statement {
                            declaration,
                            ..
                        } = export.as_ref()
                            && let ParsedStatement::VariableDeclaration(var) = declaration.as_ref()
                        {
                            if ctx.symbols.get(&var.name).is_none() {
                                let ty = var
                                    .declared_type
                                    .as_ref()
                                    .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                                    .unwrap_or(surge_ts_types::Type::Unknown);
                                ctx.symbols.insert(
                                    var.name.clone(),
                                    crate::symbols::SymbolInfo {
                                        kind: if matches!(
                                            var.kind,
                                            surge_ts_syntax::ParsedVariableKind::Const
                                        ) {
                                            crate::symbols::SymbolKind::Const
                                        } else {
                                            crate::symbols::SymbolKind::Let
                                        },
                                        ty,
                                        function_signature: None,
                                    },
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }

            let mut temp_file = parsed_file.clone();
            temp_file.statements = module.statements.clone();
            let current_type_declarations = std::mem::take(&mut ctx.type_declarations);
            let current_symbols = std::mem::take(&mut ctx.symbols);
            let raw_export_table = build_module_export_table(
                &temp_file,
                &current_type_declarations,
                &current_symbols,
                &SymbolTable::new(),
                Some(current_type_declarations_scope.clone()),
                ctx,
            );
            let lowered_type_declarations = current_type_declarations.len() as u64;
            ctx.type_declarations = current_type_declarations;
            ctx.symbols = current_symbols;

            if parsed_file.is_module {
                // `declare module "x"` inside a module file augments an existing
                // module rather than declaring a new ambient one. It is merged
                // into the resolved target on import, never made resolvable here.
                match Arc::make_mut(&mut ctx.module_augmentations).get_mut(&module.module_specifier)
                {
                    Some(existing) => merge_module_export_tables(existing, &raw_export_table),
                    None => {
                        Arc::make_mut(&mut ctx.module_augmentations)
                            .insert(module.module_specifier.clone(), raw_export_table);
                    }
                }
            } else if let Some(existing_index) = ambient_module_indexes
                .get(&module.module_specifier)
                .copied()
            {
                merge_module_export_tables(
                    &mut ambient_module_entries[existing_index].raw_export_table,
                    &raw_export_table,
                );
                if let Some(existing_table) =
                    Arc::make_mut(&mut ctx.ambient_modules).get_mut(&module.module_specifier)
                {
                    merge_module_export_tables(existing_table, &raw_export_table);
                }
            } else {
                Arc::make_mut(&mut ctx.ambient_modules)
                    .insert(module.module_specifier.clone(), raw_export_table.clone());
                ambient_module_indexes.insert(
                    module.module_specifier.clone(),
                    ambient_module_entries.len(),
                );
                ambient_module_entries.push(AmbientModuleEntry {
                    module_specifier: module.module_specifier.clone(),
                    file: temp_file,
                    raw_export_table,
                    block_scope: current_type_declarations_scope.clone(),
                });
            }

            ctx.type_declarations = saved_type_declarations;
            ctx.symbols = saved_symbols;
            let collect_duration = collect_start.elapsed();
            record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
                metrics.collect_type_declarations_passes += 1;
                metrics.lowered_type_declarations += lowered_type_declarations;
                metrics.collect_type_declarations_duration += collect_duration;
            });
        }
        ctx.type_declaration_scope = saved_type_declaration_scope;
    }

    if ambient_module_entries.is_empty() {
        return;
    }

    let ambient_files = ambient_module_entries
        .iter()
        .map(|entry| entry.file.clone())
        .collect::<Vec<_>>();
    let local_module_export_tables = ambient_module_entries
        .iter()
        .map(|entry| Some(entry.raw_export_table.clone()))
        .collect::<Vec<_>>();

    let mut resolved_module_export_tables = vec![None; ambient_module_entries.len()];
    let mut resolving = vec![false; ambient_module_entries.len()];

    for (file_index, entry) in ambient_module_entries.iter().enumerate() {
        if let Some(resolved_export_table) = crate::modules::resolve_module_export_table(
            file_index,
            &ambient_files,
            &local_module_export_tables,
            &mut resolved_module_export_tables,
            &mut resolving,
            ctx,
        ) {
            Arc::make_mut(&mut ctx.ambient_modules)
                .insert(entry.module_specifier.clone(), resolved_export_table);
        }
    }

    // Opt-in `SURGE_AMBIENT_BLOCK_IMPORTS=1`: bind each block's own imports
    // (`import { Socket } from "node:net"` inside `declare module "http"`) now
    // that every ambient specifier is registered, and publish a per-file scope
    // of block declarations + import bindings. The layered lookup's per-file
    // fallback consults it when the installed block scope misses, which is the
    // only way a block-internal import can be seen from a declaration body.
    // Import-resolution diagnostics are dropped: these imports were never
    // resolved before, and an unresolvable one must keep missing silently
    // exactly as it always has.
    //
    // Not the default yet: with the imports bound, `net.Socket extends
    // stream.Duplex` resolves — but `export =` namespace flattening registers
    // `Stream.Duplex` under its BARE name, so the namespace-member prefix
    // stack never activates and Duplex's bare sibling references (`Readable`,
    // `ArrayOptions`) still miss. The half-resolved chain drops Readable's
    // members from Socket and introduces a `Socket.destroy` TS2339 false
    // positive on tRPC. Landing this by default needs the flattening to
    // preserve the qualified declared name (or bare dual-keying of top-level
    // namespace members) first.
    if !ambient_block_imports_enabled() {
        record_program_timing(timings, |timings| {
            timings.ambient_module_binding += ambient_binding_start.elapsed()
        });
        return;
    }
    let mut ambient_file_layers: surge_ts_types::fx::FxHashMap<
        Arc<str>,
        Vec<Arc<crate::symbols::TypeDeclarationTable>>,
    > = surge_ts_types::fx::FxHashMap::default();
    for entry in &ambient_module_entries {
        let layers = ambient_file_layers
            .entry(Arc::from(entry.file.file_name.as_str()))
            .or_default();
        layers.extend(entry.block_scope.layers().iter().cloned());
        let has_imports = entry
            .file
            .statements
            .iter()
            .any(|statement| matches!(statement, ParsedStatement::ImportDeclaration(_)));
        if !has_imports {
            continue;
        }
        ctx.set_file_name(entry.file.file_name.clone());
        let diagnostics_before = ctx.diagnostics().len();
        let bindings =
            crate::modules::resolve_module_imports(&entry.file, &[], &[], &[], &|_| false, ctx);
        ctx.truncate_diagnostics(diagnostics_before);
        layers.extend(bindings.scope_layers());
    }
    ctx.ambient_file_type_scopes = Arc::new(
        ambient_file_layers
            .into_iter()
            .map(|(file_name, layers)| {
                (
                    file_name,
                    Arc::new(crate::symbols::TypeDeclarationScope::new(layers)),
                )
            })
            .collect(),
    );

    record_program_timing(timings, |timings| {
        timings.ambient_module_binding += ambient_binding_start.elapsed()
    });
}

/// Merge a module augmentation into an already-resolved target export table.
///
/// Augmented interfaces merge their members into the target's existing exports
/// (declaration merging); new exported values and types are added. The target's
/// namespace export shape is preserved, since the augmentation only extends it.
pub(crate) fn apply_module_augmentation(
    base: &mut ModuleExportTable,
    augmentation: &ModuleExportTable,
) {
    for (name, declaration) in augmentation.type_declarations.iter() {
        crate::symbols::merge_type_declaration_into_table(
            Arc::make_mut(&mut base.type_declarations),
            name.as_ref(),
            declaration,
        );
    }

    for (name, symbol) in augmentation.symbols.iter_shared() {
        if base.symbols.get(name).is_none() {
            let _ = base.symbols.insert_shared(name.clone(), symbol.clone());
        }
    }
}

pub(crate) fn merge_module_export_tables(
    target: &mut ModuleExportTable,
    source: &ModuleExportTable,
) {
    record_type_declaration_table_merge(
        None,
        source.type_declarations.len(),
        TableMergeKind::General,
    );
    for (name, declaration) in source.type_declarations.iter() {
        crate::symbols::merge_type_declaration_into_table(
            Arc::make_mut(&mut target.type_declarations),
            name.as_ref(),
            declaration,
        );
    }

    for (name, symbol) in source.symbols.iter_shared() {
        if target.symbols.get(name).is_none() {
            let _ = target.symbols.insert_shared(name.clone(), symbol.clone());
        }
    }

    if target.default_symbol.is_none() {
        target.default_symbol = source.default_symbol.clone();
    }

    target.namespace_export_object_type = None;
    target.has_unresolved_star_export |= source.has_unresolved_star_export;
    target.has_incomplete_declaration_surface |= source.has_incomplete_declaration_surface;
}
