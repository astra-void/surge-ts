use crate::{Diagnostic, TextSpan};

pub fn render_diagnostics(diagnostics: &[Diagnostic], source_text: &str) -> String {
    if diagnostics.is_empty() {
        return "No errors.".to_string();
    }

    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.render(source_text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn render_with_span(
    diagnostic: &Diagnostic,
    source_text: &str,
    span: TextSpan,
) -> String {
    let (line, column) = line_col_from_offset(source_text, span.start);
    let source_line = line_text_at_offset(source_text, span.start);
    let line_number_width = line.to_string().len();
    let line_padding = " ".repeat(line_number_width);
    let caret_width = span.end.saturating_sub(span.start).max(1);
    let caret_line = format!(
        "{}{}",
        " ".repeat(column.saturating_sub(1)),
        "^".repeat(caret_width)
    );

    format!(
        "{}[{}]: {}\n --> {}:{}:{}\n  |\n{line:>width$} | {source_line}\n{sep:>width$} | {caret_line}",
        diagnostic.severity.label(),
        diagnostic.code,
        diagnostic.message,
        diagnostic.file_name,
        line,
        column,
        width = line_number_width,
        sep = line_padding,
    )
}

fn line_col_from_offset(source_text: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    let target = offset.min(source_text.len());

    for (byte_index, ch) in source_text.char_indices() {
        if byte_index >= target {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

fn line_text_at_offset(source_text: &str, offset: usize) -> &str {
    let target = offset.min(source_text.len());
    let mut line_start = 0usize;
    let mut line_end = source_text.len();

    for (byte_index, ch) in source_text.char_indices() {
        if byte_index >= target {
            break;
        }

        if ch == '\n' {
            line_start = byte_index + ch.len_utf8();
        }
    }

    for (byte_index, ch) in source_text[line_start..].char_indices() {
        if ch == '\n' {
            line_end = line_start + byte_index;
            break;
        }
    }

    &source_text[line_start..line_end]
}
