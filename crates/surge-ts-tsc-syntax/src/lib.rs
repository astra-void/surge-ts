//! The syntactic diagnostics tsc reports for a file: a port of typescript-go's
//! scanner and recovering parser (`internal/scanner`, `internal/parser`), kept
//! to the parts that decide which errors are reported and where.
//!
//! tsc reports a program's syntactic diagnostics alone when there are any, so
//! the syntax tree built here is only ever used to reproduce the parser's own
//! decisions; nothing downstream reads it.

mod ast;
mod chars;
mod flags;
mod kind;
mod messages;
mod parser;
mod scanner;

pub struct Message {
    pub code: u32,
    pub text: &'static str,
}

#[derive(Clone)]
pub struct Diagnostic {
    pub start: usize,
    pub end: usize,
    pub message: &'static Message,
    pub args: Vec<String>,
}

/// A reported syntax error: byte offsets into the source text, tsc's code,
/// and its message with the arguments filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub start: usize,
    pub end: usize,
    pub code: u32,
    pub message: String,
}

impl Diagnostic {
    fn render(&self) -> SyntaxDiagnostic {
        let mut message = self.message.text.to_string();
        for (index, arg) in self.args.iter().enumerate() {
            message = message.replace(&format!("{{{index}}}"), arg);
        }
        SyntaxDiagnostic { start: self.start, end: self.end, code: self.message.code, message }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScriptKind {
    Ts,
    Tsx,
    Js,
    Jsx,
}

impl ScriptKind {
    /// The kind tsc gives a file by its extension; `None` for JSON and
    /// anything tsc does not parse as script.
    pub fn from_file_name(file_name: &str) -> Option<Self> {
        let lower = file_name.to_ascii_lowercase();
        if lower.ends_with(".tsx") {
            Some(Self::Tsx)
        } else if lower.ends_with(".ts") || lower.ends_with(".mts") || lower.ends_with(".cts") {
            Some(Self::Ts)
        } else if lower.ends_with(".jsx") {
            Some(Self::Jsx)
        } else if lower.ends_with(".js") || lower.ends_with(".mjs") || lower.ends_with(".cjs") {
            Some(Self::Js)
        } else {
            None
        }
    }
}

pub struct ParseOptions {
    pub script_kind: ScriptKind,
    pub is_declaration_file: bool,
    /// Whether the file is a module whatever its statements say
    /// (`moduleDetection: force`, or an `.mts`/`.cts`/`.mjs`/`.cjs` file).
    pub force_module: bool,
}

impl ParseOptions {
    pub fn for_file_name(file_name: &str) -> Option<Self> {
        let script_kind = ScriptKind::from_file_name(file_name)?;
        let lower = file_name.to_ascii_lowercase();
        let is_declaration_file = lower.ends_with(".d.ts")
            || lower.ends_with(".d.mts")
            || lower.ends_with(".d.cts")
            || lower.contains(".d.") && lower.ends_with(".ts");
        let force_module = [".mts", ".cts", ".mjs", ".cjs"].iter().any(|ext| lower.ends_with(ext));
        Some(Self { script_kind, is_declaration_file, force_module })
    }
}

/// tsc's `GetSyntacticDiagnostics` for one file: the parser's diagnostics,
/// then, for a JavaScript file, the ones it reports for TypeScript-only syntax.
///
/// Identical diagnostics are reported once, as tsc's
/// `SortAndDeduplicateDiagnostics` does: an unclosed JSX element nested in
/// another reports the missing `</` at the end of the file for both.
pub fn syntactic_diagnostics(text: &str, options: &ParseOptions) -> Vec<SyntaxDiagnostic> {
    let (diagnostics, js_diagnostics) = parser::parse(text, options);
    let mut rendered: Vec<SyntaxDiagnostic> = Vec::new();
    for diagnostic in diagnostics.iter().chain(js_diagnostics.iter()).map(Diagnostic::render) {
        if !rendered.contains(&diagnostic) {
            rendered.push(diagnostic);
        }
    }
    rendered
}
