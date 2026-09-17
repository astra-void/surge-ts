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
        } else if let Some(suggestion) = suggested_type_name(&named_type.name, ctx) {
            Diagnostic::ts2552(&named_type.name, suggestion, ctx.file_name.clone())
        } else {
            Diagnostic::ts2304(&named_type.name, ctx.file_name.clone())
        };
    let mut diagnostic = diagnostic;
    if let Some(span) = named_type.span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push_utility_diagnostic_once(diagnostic);
}

/// The closest declared type name, as tsc's `resolveNameHelper` suggests for a
/// type reference. A qualified or file-keyed table entry is not a name a bare
/// reference could have meant.
fn suggested_type_name(name: &str, ctx: &CheckerContext) -> Option<String> {
    if name.contains('.') {
        return None;
    }
    let mut candidates: Vec<&str> = ctx
        .type_declarations
        .iter()
        .chain(ctx.ambient_global_type_declarations.iter())
        .map(|(candidate, _)| candidate.as_ref())
        .filter(|candidate| !candidate.contains(['.', '\0']))
        .chain(
            ctx.type_parameter_scopes
                .iter()
                .flat_map(|scope| scope.keys().map(String::as_str)),
        )
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    crate::checks::expr::spelling_suggestion(name, candidates, 0).map(str::to_string)
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
///
/// `constraint_name` is the constraint *as written* where one can be rendered:
/// tsc names `keyof User` that way, not as the literal union it expands to, and
/// `Pick`'s own check in `utility.rs` already reports the written form. Two
/// spellings of one constraint are two dedup keys, so the same violation was
/// reported twice.
pub(crate) fn emit_type_argument_constraint(
    argument: &surge_ts_types::Type,
    constraint_name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic =
        Diagnostic::ts2344(&argument.name(), constraint_name, ctx.file_name.clone());
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
