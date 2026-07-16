use crate::{Diagnostic, LineIndex, TextSpan};

pub fn render_diagnostics(diagnostics: &[Diagnostic], source_text: &str) -> String {
    if diagnostics.is_empty() {
        return "No errors.".to_string();
    }

    let line_index = LineIndex::new(source_text);
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.render_with_line_index(source_text, &line_index))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn render_with_span(
    diagnostic: &Diagnostic,
    source_text: &str,
    line_index: &LineIndex,
    span: TextSpan,
) -> String {
    let (line, column) = line_index.line_col(source_text, span.start);
    let source_line = line_index.line_text(source_text, span.start);
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
