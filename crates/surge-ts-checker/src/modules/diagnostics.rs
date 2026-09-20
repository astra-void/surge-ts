//! Module-resolution diagnostic emitters and small structural predicates.

use super::*;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedImportDeclaration,
    ParsedImportKind, ParsedStatement, TextSpan,
};

use crate::context::{CheckerContext, FileKind, convert_span};
use crate::paths::canonicalize_if_exists_string;
use crate::program::ParsedProgramFile;

/// Wraps a module specifier in double quotes so TS2305 matches tsc, which
/// renders the specifier from its source text and therefore keeps the quotes
/// (e.g. `Module '"./user"'`). If the specifier is already quoted it is left
/// untouched.
fn quoted_module_specifier(module_specifier: &str) -> String {
    if module_specifier.len() >= 2
        && ((module_specifier.starts_with('"') && module_specifier.ends_with('"'))
            || (module_specifier.starts_with('\'') && module_specifier.ends_with('\'')))
    {
        return module_specifier.to_string();
    }

    format!("\"{module_specifier}\"")
}

/// Diagnostic for an unresolved module specifier, mirroring tsc: a Node
/// built-in name gets the install-@types/node hint via
/// `cannot_resolve_module_name_error_for_specific_module`; anything else falls
/// back to the generic TS2307. Side-effect imports never reach here — they use
/// TS2882.
fn unresolved_module_diagnostic(ctx: &CheckerContext, module_specifier: &str) -> Diagnostic {
    // A `.json` specifier under `resolveJsonModule: false` is not a missing
    // module — it is a module the option refuses to resolve, and tsc says so
    // with its own code and hint.
    if !ctx.options.resolve_json_module && surge_ts_syntax::is_json_file_name(module_specifier) {
        return Diagnostic::ts2732(module_specifier, ctx.file_name.clone());
    }

    cannot_resolve_module_name_error_for_specific_module(ctx, module_specifier)
        .unwrap_or_else(|| Diagnostic::ts2307(module_specifier, ctx.file_name.clone()))
}

/// A relative specifier that names an *existing* JavaScript file with no
/// adjacent declaration file is resolved by tsc — it is just untyped, which is
/// TS7016 under `noImplicitAny` and silent otherwise. Reporting TS2307 there
/// says the module is missing, which it is not. Only explicit `.js`/`.mjs`/
/// `.cjs`/`.jsx` specifiers are recognized; extensionless resolution stays with
/// the module loader.
fn untyped_javascript_module_path(ctx: &CheckerContext, module_specifier: &str) -> Option<String> {
    if !module_specifier.starts_with('.') {
        return None;
    }
    let extension = Path::new(module_specifier).extension()?.to_str()?;
    if !matches!(extension, "js" | "mjs" | "cjs" | "jsx") {
        return None;
    }
    let resolved = Path::new(ctx.file_name.as_str())
        .parent()?
        .join(module_specifier);
    let resolved = canonicalize_if_exists_string(&resolved);
    if !Path::new(&resolved).is_file() {
        return None;
    }
    let declaration = Path::new(&resolved).with_extension(match extension {
        "mjs" => "d.mts",
        "cjs" => "d.cts",
        _ => "d.ts",
    });
    if declaration.is_file() {
        return None;
    }
    Some(resolved)
}

/// Pushes the right diagnostic for a specifier the module loader did not
/// resolve: TS7016 (or silence) for an existing untyped JavaScript file, and the
/// caller's unresolved-module diagnostic otherwise.
fn push_untyped_javascript_module_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    span: Option<TextSpan>,
) -> bool {
    let Some(resolved) = untyped_javascript_module_path(ctx, module_specifier) else {
        return false;
    };
    if ctx.options.no_implicit_any {
        let mut diagnostic = Diagnostic::ts7016(module_specifier, &resolved, ctx.file_name.clone());
        if let Some(span) = span {
            diagnostic = diagnostic.with_span(convert_span(span));
        }
        ctx.push(diagnostic);
    }
    true
}

pub(crate) fn emit_unresolved_export_module_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    module_specifier_span: Option<TextSpan>,
) {
    if push_untyped_javascript_module_diagnostic(ctx, module_specifier, module_specifier_span) {
        return;
    }
    let mut diagnostic = unresolved_module_diagnostic(ctx, module_specifier);

    if let Some(span) = module_specifier_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn emit_unresolved_module_diagnostic(
    ctx: &mut CheckerContext,
    import: &ParsedImportDeclaration,
) {
    if !matches!(import.kind, ParsedImportKind::SideEffect)
        && push_untyped_javascript_module_diagnostic(
            ctx,
            &import.module_specifier,
            import.module_specifier_span.or(import.span),
        )
    {
        return;
    }
    let mut diagnostic = match &import.kind {
        ParsedImportKind::SideEffect => {
            Diagnostic::ts2882(&import.module_specifier, ctx.file_name.clone())
        }
        _ => unresolved_module_diagnostic(ctx, &import.module_specifier),
    };

    if let Some(span) = import.module_specifier_span.or(import.span) {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

thread_local! {
    // Whether a module has any `export * from "X"` whose target does not resolve.
    // This depends only on the (fixed-per-run) file set, but the import/export
    // binding fixpoint queries it once per importing specifier across several
    // passes. A barrel that re-exports N modules is imported by O(N) files, so the
    // uncached scan (O(re-exports) per query, each rebuilding candidate paths and
    // probing them) made the binding phase O(N^2). Keyed by the global file index;
    // cleared per run alongside the relative-module cache.
    static STAR_EXPORT_UNRESOLVED_CACHE: RefCell<HashMap<usize, bool>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn clear_star_export_unresolved_cache() {
    STAR_EXPORT_UNRESOLVED_CACHE.with(|cache| cache.borrow_mut().clear());
}

pub(crate) fn module_has_unresolved_star_export(
    file_index: usize,
    parsed_files: &[ParsedProgramFile],
    file_index_by_identity: &surge_ts_types::fx::FxHashMap<Arc<str>, usize>,
) -> bool {
    if let Some(cached) =
        STAR_EXPORT_UNRESOLVED_CACHE.with(|cache| cache.borrow().get(&file_index).copied())
    {
        return cached;
    }

    let result = parsed_files[file_index].statements.iter().any(|statement| {
        let ParsedStatement::ExportDeclaration(export) = statement else {
            return false;
        };
        let ParsedExportDeclaration::All {
            module_specifier, ..
        } = export.as_ref()
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
    });

    STAR_EXPORT_UNRESOLVED_CACHE.with(|cache| {
        cache.borrow_mut().insert(file_index, result);
    });
    result
}

pub(crate) fn should_bind_unknown_for_missing_export(
    export_table: &ModuleExportTable,
    resolved_index: Option<usize>,
    parsed_files: &[ParsedProgramFile],
) -> bool {
    let Some(file_index) = resolved_index else {
        return false;
    };
    let Some(file) = parsed_files.get(file_index) else {
        return false;
    };

    // A `.json` file whose contents did not parse is a module whose export
    // names are unknowable, not one that is missing the name asked for. surge
    // reports no JSON syntax errors, so the alternative would be a TS2305 on
    // every import of it.
    if matches!(
        file.json_module_type,
        Some(surge_ts_syntax::ParsedType::Unknown)
    ) {
        return true;
    }

    matches!(file.file_kind, FileKind::DependencyDeclaration)
        && export_table.has_incomplete_declaration_surface
}

pub(crate) fn module_has_incomplete_declaration_surface(parsed_file: &ParsedProgramFile) -> bool {
    if !parsed_file.file_kind.is_declaration() {
        return false;
    }

    parsed_file
        .statements
        .iter()
        .any(statement_has_unsupported_declaration_surface)
}

pub(crate) fn statement_has_unsupported_declaration_surface(statement: &ParsedStatement) -> bool {
    match statement {
        ParsedStatement::UnsupportedDeclaration { .. } => true,
        ParsedStatement::ImportDeclaration(import) => {
            matches!(
                import.kind,
                ParsedImportKind::Unsupported | ParsedImportKind::TypeOnlyDefault { .. }
            )
        }
        ParsedStatement::ExportDeclaration(export) => matches!(
            export.as_ref(),
            ParsedExportDeclaration::Unsupported { .. }
                | ParsedExportDeclaration::EqualsExpression { .. }
                | ParsedExportDeclaration::Default {
                    declaration: ParsedDefaultExportDeclaration::Unsupported { .. },
                    ..
                }
        ),
        ParsedStatement::DeclareModuleDeclaration(module) => module
            .statements
            .iter()
            .any(statement_has_unsupported_declaration_surface),
        _ => false,
    }
}

pub(crate) fn emit_unsupported_module_syntax_diagnostic(
    ctx: &mut CheckerContext,
    import: &ParsedImportDeclaration,
) {
    let mut diagnostic = Diagnostic::surge_unsupported_module_syntax(ctx.file_name.clone());

    if let Some(span) = import.span.or(import.module_specifier_span) {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn emit_missing_export_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    export_name: &str,
    name_span: Option<TextSpan>,
) {
    // tsc renders the module specifier from its source text, which keeps the
    // surrounding quotes (e.g. Module '"./user"'). The checker stores it
    // unquoted, so re-wrap it here.
    let specifier = quoted_module_specifier(module_specifier);
    let mut diagnostic = Diagnostic::ts2305(specifier, export_name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

/// An import naming something the resolved source module declares at its top
/// level without exporting it (TS2459, or TS2614 when the module has a default
/// export). Decided from the module's syntax only when that is conclusive: a
/// source file whose export list has no `export *`, `export =` or form surge
/// does not parse. surge's export tables also carry a module's local values, so
/// without this the import silently bound the local.
pub(crate) fn imported_name_is_unexported_local(
    resolved_index: Option<usize>,
    program_files: &[ParsedProgramFile],
    name: &str,
) -> bool {
    let Some(exported) = syntactic_export_names(resolved_index, program_files) else {
        return false;
    };
    let Some(file) = resolved_index.and_then(|index| program_files.get(index)) else {
        return false;
    };
    !exported.contains(&name)
        && crate::program::module_scope_declared_names(&file.statements).contains(name)
}

/// The names a TypeScript source module exports, in declaration order, when
/// its syntax says so conclusively: no `export *`, `export =` or export form
/// surge does not parse.
pub(crate) fn syntactic_export_names(
    resolved_index: Option<usize>,
    program_files: &[ParsedProgramFile],
) -> Option<Vec<&str>> {
    let file = resolved_index.and_then(|index| program_files.get(index))?;
    if file.file_kind.is_declaration()
        || ![".ts", ".tsx", ".mts", ".cts"]
            .iter()
            .any(|extension| file.file_name.ends_with(extension))
    {
        return None;
    }
    let mut exported: Vec<&str> = Vec::new();
    for statement in &file.statements {
        let ParsedStatement::ExportDeclaration(export) = statement else {
            continue;
        };
        match export.as_ref() {
            ParsedExportDeclaration::Statement { declaration, .. } => {
                let mut names: Vec<&str> = crate::program::module_scope_declared_names(
                    std::slice::from_ref(declaration.as_ref()),
                )
                .into_iter()
                .collect();
                names.sort_unstable();
                exported.extend(names);
            }
            ParsedExportDeclaration::Named { specifiers, .. } => {
                exported.extend(specifiers.iter().map(|specifier| specifier.exported_name.as_str()));
            }
            ParsedExportDeclaration::Default { .. } => exported.push("default"),
            ParsedExportDeclaration::Namespace { exported_name, .. } => {
                exported.push(exported_name.as_str());
            }
            ParsedExportDeclaration::Empty { .. } => {}
            ParsedExportDeclaration::All { .. }
            | ParsedExportDeclaration::NamespaceExport { .. }
            | ParsedExportDeclaration::Equals { .. }
            | ParsedExportDeclaration::EqualsExpression { .. }
            | ParsedExportDeclaration::Unsupported { .. } => return None,
        }
    }
    Some(exported)
}

/// tsc's `errorNoModuleMemberSymbol` for a named import the module does not
/// export: the closest exported name (TS2724), else TS2614 when the module
/// has a default export, else TS2305.
pub(crate) fn emit_missing_import_member_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    name: &str,
    name_span: Option<TextSpan>,
    resolved_index: Option<usize>,
    program_files: &[ParsedProgramFile],
) {
    let suggestion = syntactic_export_names(resolved_index, program_files).and_then(|exported| {
        crate::checks::expr::spelling_suggestion(
            name,
            exported.into_iter().filter(|export| *export != "default"),
            0,
        )
        .map(str::to_string)
    });
    let Some(suggestion) = suggestion else {
        emit_missing_named_import_diagnostic(
            ctx,
            module_specifier,
            name,
            name_span,
            module_has_explicit_default_export(module_specifier, resolved_index, program_files, ctx),
        );
        return;
    };
    let diagnostic = Diagnostic::ts2724(
        quoted_module_specifier(module_specifier),
        name,
        suggestion,
        ctx.file_name.clone(),
    );
    ctx.push(match name_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
}

pub(crate) fn emit_unexported_local_import_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    name: &str,
    name_span: Option<TextSpan>,
    has_explicit_default_export: bool,
) {
    let specifier = quoted_module_specifier(module_specifier);
    let diagnostic = if has_explicit_default_export {
        Diagnostic::ts2614(specifier, name, ctx.file_name.clone())
    } else {
        Diagnostic::ts2459(specifier, name, ctx.file_name.clone())
    };
    ctx.push(match name_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
}

/// tsc's `reportNonDefaultExport` for a default import from a module with no
/// default export: TS2613 when the module exports a member of the imported
/// name, TS1192 otherwise, naming the module by its resolved file (without
/// extension) when it is known. A declaration file can always be imported
/// synthetically (`canHaveSyntheticDefault`), so importing from one, or from
/// an ambient module surge has no file for, is not an error.
pub(crate) fn emit_no_default_export_diagnostic(
    ctx: &mut CheckerContext,
    local_name: &str,
    name_span: Option<TextSpan>,
    resolved_index: Option<usize>,
    program_files: &[ParsedProgramFile],
) {
    let Some(file) = resolved_index.and_then(|index| program_files.get(index)) else {
        return;
    };
    if file.file_kind.is_declaration() {
        return;
    }
    let path = file.file_name.as_str();
    let stem = [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx"]
        .iter()
        .find_map(|extension| path.strip_suffix(extension))
        .unwrap_or(path);
    let module_name = format!("\"{stem}\"");
    let exports_local_name = syntactic_export_names(resolved_index, program_files)
        .is_some_and(|names| names.contains(&local_name));
    let diagnostic = if exports_local_name {
        // The catalog has no brace escape, so the import's braces ride in the
        // argument.
        Diagnostic::ts2613(&module_name, format!("{{ {local_name} }}"), ctx.file_name.clone())
    } else {
        Diagnostic::ts1192(module_name, ctx.file_name.clone())
    };
    ctx.push(match name_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
}

pub(crate) fn emit_missing_named_import_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    export_name: &str,
    name_span: Option<TextSpan>,
    has_explicit_default_export: bool,
) {
    // See emit_missing_export_diagnostic: tsc keeps the specifier's quotes.
    let specifier = quoted_module_specifier(module_specifier);
    let mut diagnostic = if has_explicit_default_export {
        Diagnostic::ts2614(specifier, export_name, ctx.file_name.clone())
    } else {
        Diagnostic::ts2305(specifier, export_name, ctx.file_name.clone())
    };

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn module_has_explicit_default_export(
    module_specifier: &str,
    resolved_index: Option<usize>,
    program_files: &[ParsedProgramFile],
    ctx: &CheckerContext,
) -> bool {
    if program_files.iter().any(|file| {
        file.file_kind == FileKind::DependencyDeclaration
            && file.file_name.contains(module_specifier)
            && file.has_export_default
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

    let Some(resolved_file_name) = ctx
        .options
        .resolved_module_for(&ctx.file_name, module_specifier)
    else {
        return false;
    };

    let canonical_file_name = canonicalize_if_exists_string(Path::new(resolved_file_name));
    program_files
        .iter()
        .find(|file| file.file_name == canonical_file_name)
        .is_some_and(|file| file_has_explicit_default_export(file))
}

pub(crate) fn file_has_explicit_default_export(file: &ParsedProgramFile) -> bool {
    file.has_export_default
}

pub(crate) fn allows_synthetic_default_import(
    ctx: &CheckerContext,
    resolved_index: Option<usize>,
    parsed_files: &[ParsedProgramFile],
) -> bool {
    let Some(resolved_index) = resolved_index else {
        return ctx.options.allow_synthetic_default_imports();
    };

    let Some(file) = parsed_files.get(resolved_index) else {
        return false;
    };

    if file.file_kind == FileKind::DependencyDeclaration {
        return true;
    }

    false
}

pub(crate) fn push_unresolved_export_diagnostic(
    ctx: &mut CheckerContext,
    local_name: &str,
    name_span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::ts2304(local_name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    // `CheckerContext::push` already dedups by (code, file, message, span) via
    // its O(1) key index, so no pre-scan of the accumulated diagnostics is
    // needed here.
    ctx.push(diagnostic);
}
