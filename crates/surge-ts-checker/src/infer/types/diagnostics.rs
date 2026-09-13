//! Type-resolution diagnostic emitters (unknown name, arity, cycles).

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedNamedType, TextSpan};

use crate::context::{CheckerContext, convert_span};

pub(crate) fn emit_unknown_type_name(named_type: &ParsedNamedType, ctx: &mut CheckerContext) {
    if ctx.suppress_unknown_type_name() {
        return;
    }
    let diagnostic =
        if named_type.name == "Buffer" && !ctx.options.types.iter().any(|ty| ty == "node") {
            if ctx.options.types_uses_wildcard() {
                Diagnostic::ts2580(&named_type.name, ctx.file_name.clone())
            } else {
                Diagnostic::ts2591(&named_type.name, ctx.file_name.clone())
            }
        } else {
            Diagnostic::ts2304(&named_type.name, ctx.file_name.clone())
        };
    let mut diagnostic = diagnostic;
    if let Some(span) = named_type.span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push_utility_diagnostic_once(diagnostic);
}

pub(crate) fn emit_type_is_not_generic(
    name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic = Diagnostic::ts2315(name, ctx.file_name.clone());
    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push_utility_diagnostic_once(diagnostic);
}

pub(crate) fn emit_generic_arity(
    name: &str,
    arity: usize,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic = Diagnostic::ts2314(name, arity, ctx.file_name.clone());
    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push_utility_diagnostic_once(diagnostic);
}

/// `Foo<Bad>` where `Foo`'s parameter is constrained. Only reported when both the
/// argument and the constraint resolved to real, settled shapes: the whole point
/// of the degradation sentinel is that surge cannot tell "violates the
/// constraint" from "I could not model this", and reporting on the latter turns
/// every modelling gap into a false positive.
pub(crate) fn emit_type_argument_constraint(
    argument: &surge_ts_types::Type,
    constraint: &surge_ts_types::Type,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic =
        Diagnostic::ts2344(&argument.name(), &constraint.name(), ctx.file_name.clone());
    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push_utility_diagnostic_once(diagnostic);
}

pub(crate) fn emit_type_declaration_cycle(
    name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic = Diagnostic::surge_type_declaration_cycle(name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push_utility_diagnostic_once(diagnostic);
}

pub(crate) fn emit_type_alias_cycle(
    name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic = Diagnostic::surge_type_alias_cycle(name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push_utility_diagnostic_once(diagnostic);
}
