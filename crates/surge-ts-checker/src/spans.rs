use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::TextSpan as SyntaxTextSpan;

use crate::context::convert_span;

pub(crate) fn choose_span(
    primary: Option<SyntaxTextSpan>,
    fallback: Option<SyntaxTextSpan>,
) -> Option<SyntaxTextSpan> {
    primary.or(fallback)
}

pub(crate) fn diagnostic_with_syntax_span(
    diagnostic: Diagnostic,
    span: Option<SyntaxTextSpan>,
) -> Diagnostic {
    match span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    }
}
