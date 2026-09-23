//! Type-resolution diagnostic emitters (unknown name, arity, cycles).

use surge_ts_diagnostics::Diagnostic;
use std::sync::Arc;

use surge_ts_syntax::{ParsedNamedType, TextSpan};

use crate::context::{CheckerContext, convert_span};
use crate::symbols::TypeDeclarationInfo;

/// Returns whether the failure is one surge reports — the only case where the
/// name is known to be the source's error rather than a gap in what surge
/// resolved.
pub(crate) fn emit_unknown_type_name(
    named_type: &ParsedNamedType,
    ctx: &mut CheckerContext,
) -> bool {
    if ctx.suppress_unknown_type_name() {
        return false;
    }
    let mut diagnostic = unknown_type_name_diagnostic(&named_type.name, ctx);
    if let Some(span) = named_type.span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    let reportable = ctx.reports_diagnostic(&diagnostic);
    ctx.push_utility_diagnostic_once(diagnostic);
    reportable
}

/// tsc's `onFailedToResolveSymbol` for a type reference.
fn unknown_type_name_diagnostic(name: &str, ctx: &CheckerContext) -> Diagnostic {
    use crate::checks::expr::{UnresolvedNameSite, cannot_find_name_message};

    let file_name = ctx.file_name.clone();
    // `checkAndReportErrorForUsingNamespaceAsTypeOrValue`, then
    // `checkAndReportErrorForUsingValueAsType`.
    if is_namespace_like(name, ctx) {
        return Diagnostic::ts2709(name, file_name);
    }
    if names_plain_value(name, ctx) {
        return Diagnostic::ts2749(name, file_name);
    }
    let message = cannot_find_name_message(name, UnresolvedNameSite::Reference, ctx);
    if let Some(lib) = crate::checks::expr::suggested_lib_for_nonexistent_name(name) {
        return message.render(name, lib, file_name);
    }
    if let Some(suggestion) = suggested_type_name(name, ctx) {
        return Diagnostic::ts2552(name, suggestion, file_name);
    }
    message.render(name, "", file_name)
}

/// Whether `name` resolves with a namespace meaning. surge binds every
/// namespace as a value and publishes its members under qualified `ns.member`
/// keys, so a name heading such a key — or a namespace import — is one.
fn is_namespace_like(name: &str, ctx: &CheckerContext) -> bool {
    if ctx.is_namespace_import_binding(name) || ctx.namespace_meaning(name).is_some() {
        return true;
    }
    let heads_key = |candidate: &Arc<str>| {
        candidate
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('.'))
    };
    ctx.type_declarations.iter().any(|(key, _)| heads_key(key))
        || ctx.ambient_global_type_declarations.iter().any(|(key, _)| heads_key(key))
        || ctx.symbols.iter().any(|(key, _)| heads_key(key))
        || ctx.ambient_global_symbols.iter().any(|(key, _)| heads_key(key))
}

/// `checkAndReportErrorForUsingValueAsType`: the name is a value and not a
/// namespace.
fn names_plain_value(name: &str, ctx: &CheckerContext) -> bool {
    ctx.symbols.get(name).is_some() || ctx.ambient_global_symbols.get(name).is_some()
}

/// tsc's `resolveEntityName` for the head of a qualified type name
/// (`Head.Member…`) that surge could not resolve as a whole. The head is
/// looked up with a namespace meaning; only when nothing by that name could be
/// one is the failure reported. What the members of a real namespace are is
/// not modelled precisely enough to report anything past the head.
pub(crate) fn emit_unresolved_qualified_type_head(
    named_type: &ParsedNamedType,
    ctx: &mut CheckerContext,
) -> bool {
    if ctx.suppress_unknown_type_name()
        || !ctx.knows_file_imports()
        || crate::modules::is_declaration_file_name(&ctx.file_name)
    {
        return false;
    }
    let mut segments = named_type.name.split('.');
    let (Some(head), Some(member)) = (segments.next(), segments.next()) else {
        return false;
    };
    if let Some((qualified_head, _)) = ctx.namespace_info(head) {
        return emit_missing_namespace_member(named_type, &qualified_head, ctx);
    }
    if is_namespace_like(head, ctx)
        || ctx.is_import_binding(head)
        || ctx.is_umd_global_value_reference(head)
        || names_plain_value(head, ctx)
        || ctx
            .namespace_member_prefix_stack
            .iter()
            .any(|prefix| is_namespace_like(&format!("{prefix}.{head}"), ctx))
    {
        return false;
    }
    let file_name = ctx.file_name.clone();
    let is_type_parameter = ctx
        .type_parameter_scopes
        .iter()
        .any(|scope| scope.contains_key(head));
    // TS2713 underlines the qualified name; the rest underline its head.
    let (diagnostic, whole_name) =
        if is_type_parameter || ctx.lookup_type_declaration(head).is_some() {
            // `checkAndReportErrorForUsingTypeAsNamespace`.
            match declared_type_has_property(head, member, ctx) {
                Some(true) => (Diagnostic::ts2713(head, member, file_name), true),
                Some(false) => (Diagnostic::ts2702(head, file_name), false),
                None => return false,
            }
        } else if let Some(suggestion) = suggested_namespace_name(head, ctx) {
            (Diagnostic::ts2833(head, suggestion, file_name), false)
        } else {
            (Diagnostic::ts2503(head, file_name), false)
        };
    let mut diagnostic = diagnostic;
    if let Some(span) = named_type.span {
        let span = if whole_name {
            TextSpan {
                start: span.start,
                end: span.start + head.len() + 1 + member.len(),
            }
        } else {
            TextSpan {
                start: span.start,
                end: span.start + head.len(),
            }
        };
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    let reportable = ctx.reports_diagnostic(&diagnostic);
    ctx.push_utility_diagnostic_once(diagnostic);
    reportable
}

/// tsc's `resolveQualifiedName` failure past a namespace head: the first
/// segment the namespace's exports do not carry with the meaning needed there
/// (a namespace for a middle segment, a type for the last). Only a namespace
/// whose whole export list surge saw is judged.
fn emit_missing_namespace_member(
    named_type: &ParsedNamedType,
    qualified_head: &str,
    ctx: &mut CheckerContext,
) -> bool {
    use crate::program::{MEANING_NAMESPACE, MEANING_TYPE, MEANING_VALUE};

    let segments: Vec<&str> = named_type.name.split('.').collect();
    let mut namespace = qualified_head.to_string();
    // Byte offset of the current segment within the written name.
    let mut offset = segments[0].len() + 1;
    for (index, segment) in segments.iter().enumerate().skip(1) {
        let is_last = index + 1 == segments.len();
        let Some(info) = ctx.namespace_registry.info(&ctx.file_name, &namespace) else {
            return false;
        };
        if !info.complete {
            return false;
        }
        let needed = if is_last { MEANING_TYPE } else { MEANING_NAMESPACE };
        let meanings = info.exports.get(*segment).copied().unwrap_or(0);
        if meanings & needed != 0 {
            namespace = format!("{namespace}.{segment}");
            offset += segment.len() + 1;
            continue;
        }
        let file_name = ctx.file_name.clone();
        let segment_span = named_type.span.map(|span| TextSpan {
            start: span.start + offset,
            end: span.start + offset + segment.len(),
        });
        let suggestion = crate::checks::expr::spelling_suggestion(
            segment,
            info.exports
                .iter()
                .filter(|(_, meanings)| **meanings & needed != 0)
                .map(|(name, _)| name.as_ref()),
            0,
        )
        .map(str::to_string);
        let (diagnostic, span) = if let Some(suggestion) = suggestion {
            (
                Diagnostic::ts2724(&namespace, segment, suggestion, file_name),
                segment_span,
            )
        } else if is_last && meanings & MEANING_VALUE != 0 {
            // `canSuggestTypeof`: the qualified name reads a value.
            let whole = segments.join(".");
            let span = named_type.span.map(|span| TextSpan {
                start: span.start,
                end: span.start + whole.len(),
            });
            (Diagnostic::ts2749(whole, file_name), span)
        } else if !is_last && meanings & MEANING_TYPE != 0 {
            let next = segments[index + 1];
            let next_span = named_type.span.map(|span| {
                let start = span.start + offset + segment.len() + 1;
                TextSpan {
                    start,
                    end: start + next.len(),
                }
            });
            (Diagnostic::ts2713(segment, next, file_name), next_span)
        } else {
            (Diagnostic::ts2694(&namespace, segment, file_name), segment_span)
        };
        let diagnostic = match span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push_utility_diagnostic_once(diagnostic);
        return true;
    }
    false
}

/// A written qualified name (`N.Hidden`) that surge's type table resolves
/// although the namespace does not export the member: tsc reads only a
/// namespace's exports through a qualified name. Reports and answers whether
/// it did; references a namespace's own members make internally are left out.
pub(crate) fn report_unexported_namespace_member(
    named_type: &ParsedNamedType,
    ctx: &mut CheckerContext,
) -> bool {
    if !named_type.name.contains('.')
        || !ctx.namespace_member_prefix_stack.is_empty()
        || ctx.suppress_unknown_type_name()
        || !ctx.knows_file_imports()
        || crate::modules::is_declaration_file_name(&ctx.file_name)
    {
        return false;
    }
    let head = named_type.name.split('.').next().unwrap_or_default();
    let Some((qualified_head, _)) = ctx.namespace_info(head) else {
        return false;
    };
    emit_missing_namespace_member(named_type, &qualified_head, ctx)
}

/// Whether the declared type of `name` has a property `member`, as far as a
/// written object shape answers it; `None` for anything surge would have to
/// resolve to know.
fn declared_type_has_property(name: &str, member: &str, ctx: &CheckerContext) -> Option<bool> {
    match ctx.lookup_type_declaration(name)? {
        TypeDeclarationInfo::Interface(info) => {
            if info.body.members.iter().any(|candidate| candidate.name == member) {
                Some(true)
            } else if info.body.extends.is_empty() {
                Some(false)
            } else {
                None
            }
        }
        TypeDeclarationInfo::Alias(alias) => match &alias.body.ty {
            surge_ts_syntax::ParsedType::Object(object) => {
                Some(object.properties.iter().any(|property| property.name == member))
            }
            _ => None,
        },
    }
}

/// The closest name heading a namespace, for TS2833.
fn suggested_namespace_name(name: &str, ctx: &CheckerContext) -> Option<String> {
    let head = |key: &Arc<str>| key.split_once('.').map(|(head, _)| head.to_string());
    let mut candidates: Vec<String> = ctx
        .type_declarations
        .iter()
        .chain(ctx.ambient_global_type_declarations.iter())
        .filter_map(|(key, _)| head(key))
        .filter(|candidate| !candidate.contains('\0'))
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    crate::checks::expr::spelling_suggestion(name, candidates.iter().map(String::as_str), 0)
        .map(str::to_string)
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

/// tsc's type-argument count error: TS2314 when every parameter is required,
/// TS2707 with the range when some have defaults.
pub(crate) fn emit_generic_arity(
    name: &str,
    min_arity: usize,
    max_arity: usize,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic = if min_arity == max_arity {
        Diagnostic::ts2314(name, max_arity, ctx.file_name.clone())
    } else {
        Diagnostic::ts2707(name, min_arity, max_arity, ctx.file_name.clone())
    };
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
