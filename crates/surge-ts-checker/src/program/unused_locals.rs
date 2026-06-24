//! TS6133 for `noUnusedLocals`: a top-level import or value declaration whose
//! name is never referenced anywhere in the module (and is not exported) is
//! unused. Uses the module-wide read set collected from the full oxc AST
//! (`ParsedSource::module_reads`), which includes type-position and
//! export-specifier references, so the check is FP-free.

use std::collections::HashSet;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExportDeclaration, ParsedImportKind, ParsedStatement, TextSpan};

use crate::context::{CheckerContext, convert_span};

pub(crate) fn emit_unused_module_bindings(
    statements: &[ParsedStatement],
    module_reads: &[String],
    ctx: &mut CheckerContext,
) {
    let reads: HashSet<&str> = module_reads.iter().map(String::as_str).collect();

    // A binding re-exported via `export { x }` (no `from`) is used. Statement and
    // default exports wrap their declaration, so those names are never visited as
    // a bare top-level declaration below and need no exemption here.
    let mut exported: HashSet<&str> = HashSet::new();
    for statement in statements {
        if let ParsedStatement::ExportDeclaration(export) = statement {
            if let ParsedExportDeclaration::Named { specifiers, .. } = export.as_ref() {
                for specifier in specifiers {
                    exported.insert(specifier.local_name.as_str());
                }
            }
        }
    }

    let is_used = |name: &str| reads.contains(name) || exported.contains(name);

    for statement in statements {
        match statement {
            ParsedStatement::ImportDeclaration(import) => {
                for (name, span) in import_local_bindings(&import.kind) {
                    if !is_used(name) {
                        push_unused(name, span, ctx);
                    }
                }
            }
            ParsedStatement::VariableDeclaration(variable) if !variable.is_declare => {
                if !is_used(&variable.name) {
                    push_unused(&variable.name, variable.name_span, ctx);
                }
            }
            ParsedStatement::FunctionDeclaration(function) if !function.is_declare => {
                if !is_used(&function.name) {
                    push_unused(&function.name, function.name_span, ctx);
                }
            }
            // tsc does not report unused top-level *class* declarations under
            // noUnusedLocals (only variables, imports, and functions), so classes
            // are intentionally excluded here.
            _ => {}
        }
    }
}

fn import_local_bindings(kind: &ParsedImportKind) -> Vec<(&str, Option<TextSpan>)> {
    match kind {
        ParsedImportKind::Named { specifiers, .. } => specifiers
            .iter()
            .map(|specifier| (specifier.local_name.as_str(), specifier.name_span))
            .collect(),
        ParsedImportKind::DefaultAndNamed {
            local_name,
            name_span,
            specifiers,
            ..
        } => std::iter::once((local_name.as_str(), *name_span))
            .chain(
                specifiers
                    .iter()
                    .map(|specifier| (specifier.local_name.as_str(), specifier.name_span)),
            )
            .collect(),
        ParsedImportKind::Default {
            local_name,
            name_span,
        }
        | ParsedImportKind::Namespace {
            local_name,
            name_span,
            ..
        }
        | ParsedImportKind::Equals {
            local_name,
            name_span,
        } => vec![(local_name.as_str(), *name_span)],
        ParsedImportKind::SideEffect | ParsedImportKind::Unsupported => Vec::new(),
    }
}

fn push_unused(name: &str, span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let diagnostic = Diagnostic::ts6133(name, ctx.file_name.clone());
    let diagnostic = match span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };
    ctx.push(diagnostic);
}
