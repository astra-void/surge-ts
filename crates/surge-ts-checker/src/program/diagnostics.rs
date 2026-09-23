use std::collections::HashSet;

use surge_ts_diagnostics::Diagnostic;

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.

use crate::context::CheckerContext;
use super::ParsedProgramFile;

/// A parse failure reported the way tsc reports it when oxc classified the
/// failure: the catalogued message for its code, anchored at oxc's own span.
/// oxc's rendering is used only for a failure it left unnumbered, and for a
/// number surge does not catalog, so neither can invent a TypeScript code.
pub(crate) fn parser_error_diagnostic(
    error: &surge_ts_syntax::ParserError,
    file_name: &str,
) -> Diagnostic {
    let Some((descriptor, args)) = error.code.and_then(|code| parser_error_descriptor(code, error))
    else {
        return Diagnostic::surge_parser_error(error.message.clone(), file_name.to_string());
    };

    crate::spans::diagnostic_with_syntax_span(
        Diagnostic::from_descriptor(descriptor, args, file_name.to_string()),
        error.span,
    )
}

/// Codes whose one argument is the modifier oxc's label covers. oxc words
/// some of them differently from tsc (TS1031 on a constructor, TS1273), so
/// the argument cannot always be read back out of its message.
const MODIFIER_LABEL_CODES: &[u32] = &[1030, 1031, 1070, 1071, 1090, 1273];

fn parser_error_descriptor(
    code: u32,
    error: &surge_ts_syntax::ParserError,
) -> Option<(
    &'static surge_ts_diagnostics::DiagnosticDescriptor,
    Vec<surge_ts_diagnostics::DiagnosticArg>,
)> {
    if let Some(descriptor) = surge_ts_diagnostics::emitted_descriptor_for_number(code) {
        return Some((descriptor, Vec::new()));
    }
    let descriptor = surge_ts_diagnostics::emitted_descriptor_for_number_with_arity(code, 1)?;
    let argument = match_message_template(descriptor.message_template, &error.message)
        .and_then(|mut args| (args.len() == 1).then(|| args.remove(0)))
        .or_else(|| {
            MODIFIER_LABEL_CODES
                .contains(&code)
                .then(|| error.span_text.clone())
                .flatten()
        })?;
    Some((descriptor, vec![argument.into()]))
}

/// The arguments that fill `template`'s `{n}` placeholders to produce
/// `message`, tolerating a missing final period.
fn match_message_template(template: &str, message: &str) -> Option<Vec<String>> {
    let template = template.strip_suffix('.').unwrap_or(template);
    let message = message.strip_suffix('.').unwrap_or(message);
    let mut pieces = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let close = rest[open..].find('}')? + open;
        pieces.push(&rest[..open]);
        rest = &rest[close + 1..];
    }
    let (first, literals) = pieces.split_first().map_or((rest, &[][..]), |(f, l)| (*f, l));
    let mut remaining = message.strip_prefix(first)?;
    let mut args = Vec::new();
    for literal in literals.iter().copied().chain(std::iter::once(rest)) {
        if literal.is_empty() {
            args.push(remaining.to_string());
            remaining = "";
            continue;
        }
        let at = remaining.find(literal)?;
        args.push(remaining[..at].to_string());
        remaining = &remaining[at + literal.len()..];
    }
    (remaining.is_empty() && !pieces.is_empty()).then_some(args)
}

pub(super) fn emit_parser_diagnostics(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());

        let diagnostics: Vec<Diagnostic> = super::check_files::unclaimed_parser_errors(
            &parsed_file.parser_errors,
            &parsed_file.grammar_diagnostics,
            ctx,
        )
        .map(|error| parser_error_diagnostic(error, &parsed_file.file_name))
        .collect();
        for diagnostic in diagnostics {
            ctx.push(diagnostic);
        }
    }
}

pub(super) type DiagnosticDedupKey = (
    String,
    String,
    String,
    Option<surge_ts_diagnostics::TextSpan>,
);

pub(super) fn diagnostic_dedup_key(diagnostic: &surge_ts_diagnostics::Diagnostic) -> DiagnosticDedupKey {
    (
        diagnostic.code.to_string(),
        diagnostic.file_name.clone(),
        diagnostic.message.clone(),
        diagnostic.span,
    )
}

/// Order-preserving diagnostic dedup backed by a key set. The previous
/// implementation rescanned the whole accumulated `Vec` (and re-rendered every
/// code to a `String`) for each incoming diagnostic, so merging N files' results
/// was O(D^2) in the total diagnostic count. Hoisting the set across the merge
/// keeps it O(D).
#[derive(Default)]
pub(super) struct DiagnosticDeduper {
    pub(super) seen: HashSet<DiagnosticDedupKey>,
}

impl DiagnosticDeduper {
    pub(super) fn with_existing(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Self {
        Self {
            seen: diagnostics.iter().map(diagnostic_dedup_key).collect(),
        }
    }

    pub(super) fn extend(
        &mut self,
        diagnostics: &mut Vec<surge_ts_diagnostics::Diagnostic>,
        new_diagnostics: Vec<surge_ts_diagnostics::Diagnostic>,
    ) {
        for diagnostic in new_diagnostics {
            if self.seen.insert(diagnostic_dedup_key(&diagnostic)) {
                diagnostics.push(diagnostic);
            }
        }
    }
}

pub(super) fn extend_diagnostics_dedup(
    diagnostics: &mut Vec<surge_ts_diagnostics::Diagnostic>,
    new_diagnostics: Vec<surge_ts_diagnostics::Diagnostic>,
) {
    DiagnosticDeduper::with_existing(diagnostics).extend(diagnostics, new_diagnostics);
}

/// tsc's `getDiagnosticsWithPrecedingDirectives` plus the unused-directive
/// report that follows it: a diagnostic starting on a directive's suppressed
/// line is dropped and marks the nearest directive above it used; every
/// `@ts-expect-error` left unused is TS2578 at the directive. An `@ts-ignore`
/// never reports. Runs only for files surge checks in full: a declaration
/// file returns before it, since its statements are never checked and every
/// directive there would read as unused. A file with parse errors reports no
/// unused directive either: tsc skips semantic diagnostics once any syntax
/// error exists, and surge's checker may not have seen the file's statements.
pub(crate) fn apply_comment_directives(
    diagnostics: &mut Vec<surge_ts_diagnostics::Diagnostic>,
    directives: &[surge_ts_syntax::CommentDirective],
    has_parse_errors: bool,
    file_name: &str,
) {
    if directives.is_empty() {
        return;
    }
    let mut used = vec![false; directives.len()];
    diagnostics.retain(|diagnostic| {
        let Some(span) = diagnostic.span.as_ref() else {
            return true;
        };
        let nearest = directives
            .iter()
            .enumerate()
            .filter(|(_, directive)| {
                directive
                    .suppressed_line
                    .is_some_and(|line| span.start >= line.start && span.start <= line.end)
            })
            .max_by_key(|(_, directive)| directive.span.start);
        match nearest {
            Some((index, _)) => {
                used[index] = true;
                false
            }
            None => true,
        }
    });
    for (directive, used) in directives.iter().zip(used) {
        if used
            || has_parse_errors
            || directive.kind != surge_ts_syntax::CommentDirectiveKind::ExpectError
        {
            continue;
        }
        let diagnostic = Diagnostic::ts2578(file_name.to_string())
            .with_span(crate::context::convert_span(directive.span));
        let position = diagnostics
            .iter()
            .position(|existing| {
                existing
                    .span
                    .as_ref()
                    .is_some_and(|span| span.start > directive.span.start)
            })
            .unwrap_or(diagnostics.len());
        diagnostics.insert(position, diagnostic);
    }
}
