//! TS6133 for `noUnusedLocals`: a top-level import or value declaration whose
//! name is never referenced anywhere in the module (and is not exported) is
//! unused. Uses the module-wide read set collected from the full oxc AST
//! (`ParsedSource::module_reads`), which includes type-position and
//! export-specifier references, so the check is FP-free.

use std::collections::HashSet;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExportDeclaration, ParsedImportKind, ParsedStatement, TextSpan};

use crate::context::{CheckerContext, convert_span};

/// The names a file's JSX reads without writing them (tsc's
/// `markJsxAliasReferenced`): an element reads the factory's root — the `@jsx`
/// pragma's, else `jsxFactory`'s, else `reactNamespace`, else `React` — and a
/// fragment the fragment factory's (`@jsxFrag`, else `jsxFragmentFactory`,
/// else that default namespace, never the file's pragma) together with the
/// file's factory. The automatic runtime imports its factory from the runtime
/// module instead.
pub(crate) fn jsx_factory_reads(
    uses: &surge_ts_syntax::JsxFactoryUses,
    options: &crate::CheckerOptions,
) -> Vec<String> {
    if !uses.has_elements && !uses.has_fragments {
        return Vec::new();
    }
    let names = &options.jsx_factory_names;
    let runtime = uses.runtime_pragma.as_deref();
    let automatic = runtime != Some("classic")
        && (options.jsx_automatic_runtime
            || names.import_source.is_some()
            || uses.import_source_pragma.is_some()
            || runtime == Some("automatic"));
    if automatic {
        return Vec::new();
    }
    let default_namespace = names
        .factory
        .as_deref()
        .and_then(surge_ts_syntax::jsx_entity_root)
        .or_else(|| names.react_namespace.clone())
        .unwrap_or_else(|| "React".to_string());
    let factory = uses
        .factory_pragma
        .clone()
        .unwrap_or_else(|| default_namespace.clone());
    let mut reads = vec![factory];
    if uses.has_fragments {
        let fragment = uses
            .fragment_pragma
            .clone()
            .or_else(|| {
                names
                    .fragment_factory
                    .as_deref()
                    .and_then(surge_ts_syntax::jsx_entity_root)
            })
            .unwrap_or(default_namespace);
        // `jsxFragmentFactory: "null"` names no binding.
        if fragment != "null" {
            reads.push(fragment);
        }
    }
    reads
}

/// tsc's `reportUnusedVariables`: a declaration list of several declarations
/// none of which is read reports once (TS6199), a destructuring pattern of
/// several elements none of which is read reports once (TS6198,
/// `reportUnusedBindingElements`), and otherwise each unread name is TS6133 —
/// a nested pattern counting as read when any of its names is.
pub(crate) fn report_unused_declaration_list(
    list: &surge_ts_syntax::ParsedDeclarationList,
    is_used: &dyn Fn(&str) -> bool,
    ctx: &mut CheckerContext,
) {
    use surge_ts_syntax::ParsedDeclarationShape as Shape;

    fn unreferenced(shape: &Shape, is_used: &dyn Fn(&str) -> bool) -> bool {
        match shape {
            Shape::Omitted => true,
            Shape::Pattern { elements, .. } => elements.iter().all(|element| unreferenced(element, is_used)),
            Shape::Name { name, always_used, .. } => !always_used && !is_used(name),
        }
    }
    fn report_group(
        shapes: &[Shape],
        span: Option<TextSpan>,
        grouped: fn(String) -> Diagnostic,
        is_used: &dyn Fn(&str) -> bool,
        ctx: &mut CheckerContext,
    ) {
        if shapes.len() > 1 && shapes.iter().all(|shape| unreferenced(shape, is_used)) {
            let diagnostic = grouped(ctx.file_name.clone());
            ctx.push(match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            });
            return;
        }
        for shape in shapes {
            match shape {
                Shape::Pattern { span, elements } => {
                    report_group(elements, *span, Diagnostic::ts6198, is_used, ctx);
                }
                Shape::Name { name, span, .. } if unreferenced(shape, is_used) => {
                    push_unused(name, *span, ctx);
                }
                _ => {}
            }
        }
    }
    report_group(&list.declarations, list.span, Diagnostic::ts6199, is_used, ctx);
}

pub(crate) fn emit_unused_module_bindings(
    statements: &[ParsedStatement],
    module_reads: &[String],
    jsx_reads: &[String],
    ctx: &mut CheckerContext,
) {
    let reads: HashSet<&str> = module_reads
        .iter()
        .chain(jsx_reads)
        .map(String::as_str)
        .collect();

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

    // tsc's `reportUnusedImports`: the bindings of one import declaration are
    // reported together — as TS6192 on the declaration when it has more than
    // one and none is used. The parser splits `import d, * as ns` and type-only
    // specifiers into several declarations that share the statement's span.
    let mut import_groups: Vec<(Option<TextSpan>, usize, Vec<(&str, Option<TextSpan>)>)> = Vec::new();
    let mut declaration_lists: Vec<std::sync::Arc<surge_ts_syntax::ParsedDeclarationList>> = Vec::new();
    for statement in statements {
        match statement {
            ParsedStatement::ImportDeclaration(import) => {
                let bindings = import_local_bindings(&import.kind);
                let group = match import_groups
                    .iter()
                    .position(|(span, _, _)| span.is_some() && *span == import.span)
                {
                    Some(index) => index,
                    None => {
                        import_groups.push((import.span, 0, Vec::new()));
                        import_groups.len() - 1
                    }
                };
                import_groups[group].1 += bindings.len();
                let is_equals = matches!(import.kind, ParsedImportKind::Equals { .. });
                for (name, span) in bindings {
                    // tsc exempts an `_`-prefixed import-clause binding, the idiom
                    // for importing a name only to assert it is exported; an
                    // `import x = require()` and a plain `const _x` still report.
                    if !is_used(name) && (is_equals || !name.starts_with('_')) {
                        import_groups[group].2.push((name, span));
                    }
                }
            }
            ParsedStatement::VariableDeclaration(variable) if !variable.is_declare => {
                match &variable.declaration_list {
                    Some(list) => {
                        if !declaration_lists.iter().any(|seen| std::sync::Arc::ptr_eq(seen, list)) {
                            declaration_lists.push(list.clone());
                        }
                    }
                    // See the matching exemption in `collect_local_var_declarations`.
                    None if variable.from_binding_pattern && variable.name.starts_with('_') => {}
                    None => {
                        if !is_used(&variable.name) {
                            push_unused(&variable.name, variable.name_span, ctx);
                        }
                    }
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

    for list in &declaration_lists {
        report_unused_declaration_list(list, &is_used, ctx);
    }

    for (span, binding_count, unused) in import_groups {
        if binding_count > 1 && unused.len() == binding_count {
            let diagnostic = Diagnostic::ts6192(ctx.file_name.clone());
            ctx.push(match span {
                Some(span) => diagnostic.with_span(convert_span(span)),
                None => diagnostic,
            });
            continue;
        }
        for (name, span) in unused {
            push_unused(name, span, ctx);
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
            ..
        } => vec![(local_name.as_str(), *name_span)],
        ParsedImportKind::SideEffect
        | ParsedImportKind::Unsupported
        | ParsedImportKind::EntityAlias { .. }
        | ParsedImportKind::TypeOnlyDefault { .. } => Vec::new(),
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
