//! `@ts-expect-error` / `@ts-ignore` comment directives.
//!
//! Each directive suppresses every diagnostic reported on the next line that
//! carries code: tsc walks *backwards* from a diagnostic's line over blank and
//! `//`-comment lines looking for a directive, so an
//! `// @ts-expect-error` followed by an `// eslint-disable-next-line` still
//! suppresses the statement below both. Only the *suppressing* half is modelled: TypeScript also reports TS2578
//! for an `@ts-expect-error` that suppressed nothing, but surge under-reports,
//! so an unused directive here means "surge missed the error", not "the source
//! is wrong".

use oxc_ast::Comment;

use crate::TextSpan;

/// Byte ranges of the lines suppressed by a file's directives, in source order.
pub(crate) fn collect_suppressed_ranges(source_text: &str, comments: &[Comment]) -> Vec<TextSpan> {
    let bytes = source_text.as_bytes();
    let mut ranges: Vec<TextSpan> = Vec::new();

    for comment in comments {
        let start = comment.span.start as usize;
        let end = (comment.span.end as usize).min(bytes.len());
        if start >= end {
            continue;
        }
        let text = &source_text[start..end];
        if !text.contains("@ts-expect-error") && !text.contains("@ts-ignore") {
            continue;
        }
        let Some(range) = next_line_range(bytes, end) else {
            continue;
        };
        if ranges.last() != Some(&range) {
            ranges.push(range);
        }
    }

    ranges.sort_by_key(|range| range.start);
    ranges.dedup();
    ranges
}

/// The byte range of the first line after `offset` that is neither blank nor a
/// `//` comment — the line tsc's backwards walk stops at, and therefore the one
/// the directive suppresses. `None` when the directive is followed by no such
/// line.
fn next_line_range(bytes: &[u8], offset: usize) -> Option<TextSpan> {
    let mut index = offset;
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }

    loop {
        if index >= bytes.len() {
            return None;
        }
        let start = index + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        if !is_comment_or_blank_line(bytes, start, end) {
            return Some(TextSpan { start, end });
        }
        index = end;
    }
}

/// tsc's `isCommentOrBlankLine`: whitespace only, or the first non-whitespace
/// characters are `//`. A block comment does not count, exactly as there.
fn is_comment_or_blank_line(bytes: &[u8], start: usize, end: usize) -> bool {
    let mut index = start;
    while index < end && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    if index >= end || bytes[index] == b'\r' {
        return true;
    }
    index + 1 < end && bytes[index] == b'/' && bytes[index + 1] == b'/'
}
