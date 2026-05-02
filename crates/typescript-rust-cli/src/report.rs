use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};
use typescript_rust_config::LoadedTsConfig;
use typescript_rust_diagnostics::{Diagnostic, render_diagnostics};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct CompatReportCountEntry {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct CompatReportParserErrorEntry {
    pub file_name: String,
    pub message: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct ProjectCompatibilityReport {
    pub root_dir: String,
    pub files_loaded: usize,
    pub diagnostics_total: usize,
    pub by_code: Vec<CompatReportCountEntry>,
    pub by_file: Vec<CompatReportCountEntry>,
    pub parser_errors: Vec<CompatReportParserErrorEntry>,
}

pub fn build_project_compatibility_report(
    loaded: &LoadedTsConfig,
    diagnostics: &[Diagnostic],
) -> ProjectCompatibilityReport {
    let mut by_code = HashMap::<String, usize>::new();
    let mut by_file = HashMap::<String, usize>::new();
    let mut parser_errors = HashMap::<(String, String), usize>::new();

    for diagnostic in diagnostics {
        *by_code.entry(diagnostic.code.to_string()).or_default() += 1;
        *by_file
            .entry(report_path_label(&loaded.root_dir, &diagnostic.file_name))
            .or_default() += 1;

        if diagnostic.code.to_string() == "typescript-rust::parser-error" {
            *parser_errors
                .entry((
                    report_path_label(&loaded.root_dir, &diagnostic.file_name),
                    diagnostic.message.clone(),
                ))
                .or_default() += 1;
        }
    }

    ProjectCompatibilityReport {
        root_dir: loaded.root_dir.display().to_string(),
        files_loaded: loaded.files.len(),
        diagnostics_total: diagnostics.len(),
        by_code: sort_counts(by_code),
        by_file: sort_counts(by_file),
        parser_errors: sort_parser_errors(parser_errors),
    }
}

pub fn render_project_diagnostics_json(
    loaded: &LoadedTsConfig,
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    max_diagnostics: Option<usize>,
) -> Value {
    let limit = max_diagnostics.unwrap_or(usize::MAX);
    let diagnostics = diagnostics
        .iter()
        .take(limit)
        .map(|diagnostic| render_diagnostic_json(loaded, diagnostic, sources))
        .collect::<Vec<_>>();

    let mut root = Map::new();
    root.insert("diagnostics".to_string(), Value::Array(diagnostics));

    Value::Object(root)
}

pub fn render_project_compatibility_report_text(report: &ProjectCompatibilityReport) -> String {
    let mut lines = Vec::new();
    lines.push("Compatibility report".to_string());
    lines.push(format!("Root dir: {}", report.root_dir));
    lines.push(format!("Files loaded: {}", report.files_loaded));
    lines.push(format!("Diagnostics: {}", report.diagnostics_total));
    lines.push(String::new());
    lines.push("By code:".to_string());
    if report.by_code.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.by_code {
            lines.push(format!("{}  {}", entry.key, entry.count));
        }
    }
    lines.push(String::new());
    lines.push("By file:".to_string());
    if report.by_file.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.by_file {
            lines.push(format!("{}  {}", entry.key, entry.count));
        }
    }
    lines.push(String::new());
    lines.push(format!("Parser errors: {}", parser_error_total(report)));
    lines.push("Top parser errors:".to_string());
    if report.parser_errors.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.parser_errors {
            lines.push(format!(
                "{}: {}  {}",
                entry.file_name, entry.message, entry.count
            ));
        }
    }

    lines.join("\n")
}

pub fn render_project_compatibility_report_json(report: &ProjectCompatibilityReport) -> Value {
    let mut root = Map::new();
    root.insert(
        "rootDir".to_string(),
        Value::String(report.root_dir.clone()),
    );
    root.insert(
        "filesLoaded".to_string(),
        Value::from(report.files_loaded as u64),
    );
    root.insert(
        "diagnosticsTotal".to_string(),
        Value::from(report.diagnostics_total as u64),
    );
    root.insert(
        "byCode".to_string(),
        Value::Array(
            report
                .by_code
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("code".to_string(), Value::String(entry.key.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert(
        "byFile".to_string(),
        Value::Array(
            report
                .by_file
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("fileName".to_string(), Value::String(entry.key.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert(
        "parserErrors".to_string(),
        Value::Array(
            report
                .parser_errors
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert(
                        "fileName".to_string(),
                        Value::String(entry.file_name.clone()),
                    );
                    item.insert("message".to_string(), Value::String(entry.message.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );

    Value::Object(root)
}

pub fn render_project_diagnostics_preview(
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    show_spans: bool,
    max_diagnostics: Option<usize>,
) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    let mut diagnostics_by_file: HashMap<String, Vec<Diagnostic>> = HashMap::new();
    for diagnostic in diagnostics {
        diagnostics_by_file
            .entry(diagnostic.file_name.clone())
            .or_default()
            .push(diagnostic.clone());
    }

    let limit = max_diagnostics.unwrap_or(usize::MAX);
    let mut rendered = Vec::new();
    let mut rendered_count = 0usize;
    let mut truncated = false;

    for (file_path, file_name, source_text) in sources {
        if rendered_count >= limit {
            truncated = diagnostics.len() > rendered_count;
            break;
        }

        let Some(file_diagnostics) = diagnostics_by_file.remove(file_name) else {
            continue;
        };

        let block = render_diagnostic_block(
            file_path.as_path(),
            source_text,
            &file_diagnostics,
            show_spans,
            limit.saturating_sub(rendered_count),
            &mut rendered_count,
        );

        if !block.is_empty() {
            rendered.push(block);
        }

        if rendered_count >= limit {
            truncated = diagnostics.len() > rendered_count || !diagnostics_by_file.is_empty();
            break;
        }
    }

    if !diagnostics_by_file.is_empty() && rendered_count < limit {
        let mut remaining = diagnostics_by_file.into_iter().collect::<Vec<_>>();
        remaining.sort_by(|left, right| left.0.cmp(&right.0));

        for (file_name, file_diagnostics) in remaining {
            if rendered_count >= limit {
                truncated = diagnostics.len() > rendered_count;
                break;
            }

            let block = render_diagnostic_block(
                Path::new(&file_name),
                "",
                &file_diagnostics,
                show_spans,
                limit.saturating_sub(rendered_count),
                &mut rendered_count,
            );

            if !block.is_empty() {
                rendered.push(block);
            }
        }

        truncated = truncated || diagnostics.len() > rendered_count;
    }

    if truncated {
        rendered.push(format!(
            "Showing first {} of {} diagnostics.",
            rendered_count,
            diagnostics.len()
        ));
    }

    rendered.join("\n\n")
}

fn render_diagnostic_block(
    file_path: &Path,
    source_text: &str,
    diagnostics: &[Diagnostic],
    show_spans: bool,
    remaining_limit: usize,
    rendered_count: &mut usize,
) -> String {
    if diagnostics.is_empty() || remaining_limit == 0 {
        return String::new();
    }

    let take = remaining_limit.min(diagnostics.len());
    let diagnostics = &diagnostics[..take];
    *rendered_count += take;

    let rendered = if show_spans {
        render_diagnostics_with_spans(diagnostics, source_text)
    } else {
        render_diagnostics(diagnostics, source_text)
    };

    format!("{}\n{}", file_path.display(), rendered)
}

fn render_diagnostics_with_spans(diagnostics: &[Diagnostic], source_text: &str) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    diagnostics
        .iter()
        .map(|diagnostic| render_diagnostic_with_span(diagnostic, source_text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_diagnostic_with_span(diagnostic: &Diagnostic, source_text: &str) -> String {
    let mut header = format!("{} {}", diagnostic.code, diagnostic.file_name);

    if let Some(span) = diagnostic.span {
        let (line, column) = line_col_from_offset(source_text, span.start);
        header.push_str(&format!(
            " start={} end={} line={} column={}",
            span.start, span.end, line, column
        ));
    } else {
        header.push_str(" (no span)");
    }

    format!("{header}\n{}", diagnostic.render(source_text))
}

pub(crate) fn line_col_from_offset(source_text: &str, offset: usize) -> (usize, usize) {
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

fn render_diagnostic_json(
    loaded: &LoadedTsConfig,
    diagnostic: &Diagnostic,
    sources: &[(PathBuf, String, String)],
) -> Value {
    let mut item = Map::new();
    item.insert(
        "code".to_string(),
        Value::String(diagnostic.code.to_string()),
    );
    item.insert(
        "fileName".to_string(),
        Value::String(report_path_label(&loaded.root_dir, &diagnostic.file_name)),
    );
    item.insert(
        "message".to_string(),
        Value::String(diagnostic.message.clone()),
    );

    if let Some(span) = diagnostic.span {
        let mut span_json = Map::new();
        span_json.insert("start".to_string(), Value::from(span.start as u64));
        span_json.insert("end".to_string(), Value::from(span.end as u64));
        item.insert("span".to_string(), Value::Object(span_json));

        if let Some(source_text) = source_text_for_diagnostic(sources, &diagnostic.file_name) {
            let (line, column) = line_col_from_offset(source_text, span.start);
            item.insert("line".to_string(), Value::from(line as u64));
            item.insert("column".to_string(), Value::from(column as u64));
        }
    }

    Value::Object(item)
}

fn source_text_for_diagnostic<'a>(
    sources: &'a [(PathBuf, String, String)],
    file_name: &str,
) -> Option<&'a str> {
    sources
        .iter()
        .find(|(_, source_file_name, _)| source_file_name == file_name)
        .map(|(_, _, source_text)| source_text.as_str())
}

fn sort_counts(counts: HashMap<String, usize>) -> Vec<CompatReportCountEntry> {
    let mut entries = counts
        .into_iter()
        .map(|(key, count)| CompatReportCountEntry { key, count })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    entries
}

fn sort_parser_errors(
    counts: HashMap<(String, String), usize>,
) -> Vec<CompatReportParserErrorEntry> {
    let mut entries = counts
        .into_iter()
        .map(
            |((file_name, message), count)| CompatReportParserErrorEntry {
                file_name,
                message,
                count,
            },
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.file_name.cmp(&right.file_name))
            .then_with(|| left.message.cmp(&right.message))
    });
    entries
}

fn parser_error_total(report: &ProjectCompatibilityReport) -> usize {
    report.parser_errors.iter().map(|entry| entry.count).sum()
}

fn report_path_label(root_dir: &Path, file_name: &str) -> String {
    let path = Path::new(file_name);
    path.strip_prefix(root_dir)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| file_name.to_string())
}
