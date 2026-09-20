use oxc_span::Span;

use crate::TextSpan;

pub(crate) fn text_span_from_oxc_span(span: Span) -> TextSpan {
    TextSpan {
        start: span.start as usize,
        end: span.end as usize,
    }
}

thread_local! {
    static LOWERING_SOURCE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Makes `source_text` readable to the lowering for the duration of `f`. oxc
/// records no span for an operator token, so the one diagnostic tsc anchors
/// there has to find it in the text between the operands.
pub(crate) fn with_lowering_source<R>(source_text: &str, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            LOWERING_SOURCE.with(|source| *source.borrow_mut() = self.0.take());
        }
    }
    let previous =
        LOWERING_SOURCE.with(|source| source.borrow_mut().replace(source_text.to_string()));
    let _restore = Restore(previous);
    f()
}

/// The span of the single-character `operator` written between `left_end` and
/// `right_start`, skipping comments. `None` outside [`with_lowering_source`].
pub(crate) fn operator_token_span(
    left_end: u32,
    right_start: u32,
    operator: u8,
) -> Option<crate::TextSpan> {
    LOWERING_SOURCE.with(|source| {
        let source = source.borrow();
        let bytes = source.as_deref()?.as_bytes();
        let (start, end) = (left_end as usize, (right_start as usize).min(bytes.len()));
        let mut index = start;
        while index < end {
            match (bytes[index], bytes.get(index + 1)) {
                (b'/', Some(b'*')) => {
                    index += 2;
                    while index + 1 < end && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                        index += 1;
                    }
                    index += 2;
                }
                (b'/', Some(b'/')) => {
                    while index < end && bytes[index] != b'\n' {
                        index += 1;
                    }
                }
                (byte, _) if byte == operator => {
                    return Some(crate::TextSpan {
                        start: index,
                        end: index + 1,
                    });
                }
                _ => index += 1,
            }
        }
        None
    })
}
