use std::collections::HashSet;

use surge_ts_diagnostics::Diagnostic;

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.

use crate::context::CheckerContext;
use super::ParsedProgramFile;

pub(super) fn emit_parser_diagnostics(parsed_files: &[ParsedProgramFile], ctx: &mut CheckerContext) {
    for parsed_file in parsed_files {
        ctx.set_file_name(parsed_file.file_name.clone());

        for message in &parsed_file.parser_errors {
            ctx.push(Diagnostic::surge_parser_error(
                message.clone(),
                parsed_file.file_name.clone(),
            ));
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

/// Drops the diagnostics an `@ts-expect-error`/`@ts-ignore` directive suppresses:
/// every one whose span starts on the directive's following line. A diagnostic
/// with no span cannot be attributed to a line and is kept.
pub(crate) fn drop_suppressed_diagnostics(
    diagnostics: &mut Vec<surge_ts_diagnostics::Diagnostic>,
    suppressed_ranges: &[surge_ts_syntax::TextSpan],
) {
    if suppressed_ranges.is_empty() || diagnostics.is_empty() {
        return;
    }
    diagnostics.retain(|diagnostic| {
        let Some(span) = diagnostic.span.as_ref() else {
            return true;
        };
        !suppressed_ranges
            .iter()
            .any(|range| span.start >= range.start && span.start <= range.end)
    });
}
