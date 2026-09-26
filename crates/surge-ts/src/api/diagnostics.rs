use surge_ts_config::ConfigDiagnostic;
use surge_ts_diagnostics::DiagnosticCode;

use super::handles::SourceFileId;
use super::positions::PositionMap;
use super::program::Program;

/// TypeScript's `DiagnosticCategory`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(i32)]
pub enum DiagnosticCategory {
    Warning = 0,
    Error = 1,
    Suggestion = 2,
    Message = 3,
}

impl DiagnosticCategory {
    /// tsc's `diagnosticCategoryName`.
    pub fn name(self) -> &'static str {
        match self {
            DiagnosticCategory::Warning => "warning",
            DiagnosticCategory::Error => "error",
            DiagnosticCategory::Suggestion => "suggestion",
            DiagnosticCategory::Message => "message",
        }
    }
}

/// TypeScript's `Diagnostic`.
///
/// `start` and `length` are UTF-16 offsets into the file's text, as
/// TypeScript reports them. `message_text` is the full message, with any
/// elaboration tsc would chain already flattened into it (surge keeps no
/// message chains).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub file: Option<SourceFileId>,
    pub start: Option<u32>,
    pub length: Option<u32>,
    /// tsc's diagnostic code; `0` for a diagnostic surge reports that tsc has
    /// no code for (see [`Diagnostic::surge_code`]).
    pub code: u32,
    pub category: DiagnosticCategory,
    pub message_text: String,
    /// surge's own code for a diagnostic without a tsc code, such as a
    /// configuration problem.
    pub surge_code: Option<String>,
    /// The file a diagnostic outside the program's files is about (a config
    /// file).
    pub file_name: Option<String>,
}

impl Diagnostic {
    pub(crate) fn from_config(diagnostic: &ConfigDiagnostic) -> Self {
        Diagnostic {
            file: None,
            start: None,
            length: None,
            code: 0,
            category: DiagnosticCategory::Error,
            message_text: diagnostic.message.clone(),
            surge_code: Some(diagnostic.code.to_string()),
            file_name: Some(diagnostic.file_name.to_string_lossy().into_owned()),
        }
    }
}

pub(crate) fn convert_diagnostic(
    diagnostic: &surge_ts_diagnostics::Diagnostic,
    file: Option<SourceFileId>,
    positions: Option<&PositionMap>,
) -> Diagnostic {
    let (code, surge_code) = match diagnostic.code {
        DiagnosticCode::TypeScript(code) => (code, None),
        DiagnosticCode::Custom(code) => (0, Some(code.to_string())),
    };
    let (start, length) = match (diagnostic.span, positions) {
        (Some(span), Some(positions)) => {
            let start = positions.to_utf16(span.start);
            let end = positions.to_utf16(span.end.max(span.start));
            (Some(start), Some(end - start))
        }
        _ => (None, None),
    };
    Diagnostic {
        file,
        start,
        length,
        code,
        category: match diagnostic.severity {
            surge_ts_diagnostics::DiagnosticCategory::Error => DiagnosticCategory::Error,
            surge_ts_diagnostics::DiagnosticCategory::Warning => DiagnosticCategory::Warning,
            surge_ts_diagnostics::DiagnosticCategory::Suggestion => DiagnosticCategory::Suggestion,
            surge_ts_diagnostics::DiagnosticCategory::Message => DiagnosticCategory::Message,
        },
        message_text: diagnostic.message.clone(),
        surge_code,
        file_name: (file.is_none() && !diagnostic.file_name.is_empty()).then(|| diagnostic.file_name.clone()),
    }
}

/// tsc's `sortAndDeduplicateDiagnostics`: by file name, start, length, code
/// and message, dropping exact duplicates.
pub(crate) fn sort_and_deduplicate(program: &Program, mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let file_name = |diagnostic: &Diagnostic| match diagnostic.file {
        Some(file) => program.file_name(file).to_string(),
        None => diagnostic.file_name.clone().unwrap_or_default(),
    };
    diagnostics.sort_by(|left, right| {
        file_name(left)
            .cmp(&file_name(right))
            .then(left.start.unwrap_or(0).cmp(&right.start.unwrap_or(0)))
            .then(left.length.unwrap_or(0).cmp(&right.length.unwrap_or(0)))
            .then(left.code.cmp(&right.code))
            .then(left.message_text.cmp(&right.message_text))
    });
    diagnostics.dedup();
    diagnostics
}

/// tsc's `formatDiagnostic`: `file(line,col): category TScode: message`,
/// with `file` relative to `current_directory`.
pub fn format_diagnostic(program: &Program, diagnostic: &Diagnostic, current_directory: &str, new_line: &str) -> String {
    let mut out = String::new();
    if let Some(file) = diagnostic.file {
        let name = relative_file_name(program.file_name(file), current_directory);
        let location = program.line_and_character_of_position(file, diagnostic.start.unwrap_or(0));
        out.push_str(&format!("{name}({},{}): ", location.line + 1, location.character + 1));
    }
    out.push_str(&format!(
        "{} TS{}: {}{new_line}",
        diagnostic.category.name(),
        diagnostic.code,
        diagnostic.message_text.replace('\n', new_line)
    ));
    out
}

/// tsc's `convertToRelativePath` for display.
fn relative_file_name(file_name: &str, current_directory: &str) -> String {
    let directory = current_directory.trim_end_matches('/');
    match file_name.strip_prefix(directory).and_then(|rest| rest.strip_prefix('/')) {
        Some(relative) if !directory.is_empty() => relative.to_string(),
        _ => file_name.to_string(),
    }
}
