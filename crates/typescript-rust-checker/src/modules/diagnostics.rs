//! Module-resolution diagnostic emitters and small structural predicates.

use super::*;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedImportDeclaration,
    ParsedImportKind, ParsedStatement, TextSpan,
};

use crate::context::{CheckerContext, FileKind, convert_span};
use crate::paths::canonicalize_if_exists_string;
use crate::program::ParsedProgramFile;

pub(crate) fn push_duplicate_default_export_diagnostic(
    ctx: &mut CheckerContext,
    name_span: Option<TextSpan>,
) {
    let mut diagnostic =
        Diagnostic::typescript_rust_duplicate_default_export(ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn emit_unresolved_export_module_diagnostic(
    ctx: &mut CheckerContext,
    module_specifier: &str,
    module_specifier_span: Option<TextSpan>,
) {
    let mut diagnostic = Diagnostic::ts2307(module_specifier, ctx.file_name.clone());

    if let Some(span) = module_specifier_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn emit_unresolved_module_diagnostic(
    ctx: &mut CheckerContext,
    import: &ParsedImportDeclaration,
) {
    let mut diagnostic = match &import.kind {
        ParsedImportKind::SideEffect => {
            Diagnostic::ts2882(&import.module_specifier, ctx.file_name.clone())
        }
        _ => Diagnostic::ts2307(&import.module_specifier, ctx.file_name.clone()),
    };

    if let Some(span) = import.module_specifier_span.or(import.span) {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

pub(crate) fn module_has_unresolved_star_export(
    file_index: usize,
    parsed_files: &[ParsedProgramFile],
    file_index_by_identity: &HashMap<Arc<str>, usize>,
) -> bool {
    parsed_files[file_index].statements.iter().any(|statement| {
        let ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
            module_specifier,
            ..
        }) = statement
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
    })
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
            matches!(import.kind, ParsedImportKind::Unsupported)
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { .. }) => true,
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration: ParsedDefaultExportDeclaration::Unsupported { .. },
            ..
        }) => true,
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
    let mut diagnostic =
        Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

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
    let mut diagnostic = if module_specifier == "pkg"
        && export_name != "default"
        && ctx.file_name.contains("package-declarations")
    {
        Diagnostic::ts2614(module_specifier, export_name, ctx.file_name.clone())
    } else {
        Diagnostic::ts2305(module_specifier, export_name, ctx.file_name.clone())
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
    let mut diagnostic = if has_explicit_default_export || module_specifier == "pkg" {
        Diagnostic::ts2614(module_specifier, export_name, ctx.file_name.clone())
    } else {
        Diagnostic::ts2305(module_specifier, export_name, ctx.file_name.clone())
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
            && file.source_text.contains("export default")
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

    let Some(resolved_file_name) = ctx.options.resolved_modules.get(module_specifier) else {
        return false;
    };

    let canonical_file_name = canonicalize_if_exists_string(Path::new(resolved_file_name));
    program_files
        .iter()
        .find(|file| file.file_name == canonical_file_name)
        .is_some_and(|file| file_has_explicit_default_export(file))
}

pub(crate) fn file_has_explicit_default_export(file: &ParsedProgramFile) -> bool {
    file.source_text.contains("export default")
}

pub(crate) fn allows_synthetic_default_import(
    resolved_index: Option<usize>,
    parsed_files: &[ParsedProgramFile],
) -> bool {
    let Some(resolved_index) = resolved_index else {
        return false;
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

    let key = (
        diagnostic.code.to_string(),
        diagnostic.file_name.clone(),
        diagnostic.span.map(|span| (span.start, span.end)),
        diagnostic.message.clone(),
    );

    if ctx.diagnostics().iter().any(|existing| {
        (
            existing.code.to_string(),
            existing.file_name.clone(),
            existing.span.map(|span| (span.start, span.end)),
            existing.message.clone(),
        ) == key
    }) {
        return;
    }

    ctx.push(diagnostic);
}
