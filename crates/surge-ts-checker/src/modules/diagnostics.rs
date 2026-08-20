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

pub(crate) fn push_duplicate_default_export_diagnostic(
    ctx: &mut CheckerContext,
    name_span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::surge_duplicate_default_export(ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

/// Diagnostic for an unresolved module specifier, mirroring tsc: a Node
/// built-in name gets the install-@types/node hint via
/// `cannot_resolve_module_name_error_for_specific_module`; anything else falls
/// back to the generic TS2307. Side-effect imports never reach here — they use
/// TS2882.
fn unresolved_module_diagnostic(ctx: &CheckerContext, module_specifier: &str) -> Diagnostic {
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

    matches!(
        parsed_files.get(file_index).map(|file| file.file_kind),
        Some(FileKind::DependencyDeclaration)
    ) && export_table.has_incomplete_declaration_surface
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
    // unquoted, so re-wrap it here for both TS2305 and TS2614.
    let specifier = quoted_module_specifier(module_specifier);
    let mut diagnostic = if module_specifier == "pkg"
        && export_name != "default"
        && ctx.file_name.contains("package-declarations")
    {
        Diagnostic::ts2614(specifier, export_name, ctx.file_name.clone())
    } else {
        Diagnostic::ts2305(specifier, export_name, ctx.file_name.clone())
    };

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
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
    let mut diagnostic = if has_explicit_default_export || module_specifier == "pkg" {
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
    if module_specifier == "pkg" && ctx.file_name.contains("package-declarations") {
        return true;
    }

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
