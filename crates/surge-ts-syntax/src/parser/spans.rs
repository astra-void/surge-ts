use oxc_span::Span;

use crate::TextSpan;

pub(crate) fn text_span_from_oxc_span(span: Span) -> TextSpan {
    TextSpan {
        start: span.start as usize,
        end: span.end as usize,
    }
}
