use std::{
    collections::BTreeSet,
    error::Error,
    fmt, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DiagnosticCategory {
    Error,
    Warning,
    Suggestion,
    Message,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSupport {
    CatalogOnly,
    Emitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSource {
    Typescript,
    TypescriptRust,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CatalogEntry {
    pub code: String,
    pub category: DiagnosticCategory,
    pub message: String,
    pub source: DiagnosticSource,
    pub arity: usize,
    #[serde(default = "default_support")]
    pub support: DiagnosticSupport,
}

fn default_support() -> DiagnosticSupport {
    DiagnosticSupport::Emitted
}

#[derive(Debug)]
pub struct ValidationError(String);

impl ValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ValidationError {}

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub fn load_catalog(path: impl AsRef<Path>) -> Result<Vec<CatalogEntry>> {
    let contents = fs::read_to_string(path)?;
    let entries: Vec<CatalogEntry> = serde_json::from_str(&contents)?;
    validate_catalog(&entries)?;
    Ok(entries)
}

pub fn validate_catalog(entries: &[CatalogEntry]) -> Result<()> {
    let mut seen_codes = BTreeSet::new();
    let mut seen_function_names = BTreeSet::new();

    for entry in entries {
        if entry.code.trim().is_empty() {
            return Err(validation_error("catalog entry code must not be empty"));
        }

        if !seen_codes.insert(entry.code.clone()) {
            return Err(validation_error(format!(
                "duplicate diagnostic code: {}",
                entry.code
            )));
        }

        if entry.message.trim().is_empty() {
            return Err(validation_error(format!(
                "diagnostic {} has an empty message",
                entry.code
            )));
        }

        validate_code_policy(entry)?;

        let placeholder_arity = placeholder_arity(&entry.message)?;
        if placeholder_arity != entry.arity {
            return Err(validation_error(format!(
                "diagnostic {} declares arity {} but message requires {}",
                entry.code, entry.arity, placeholder_arity
            )));
        }

        let function_name = diagnostic_function_name(entry);
        if !seen_function_names.insert(function_name) {
            return Err(validation_error(format!(
                "duplicate generated function name for {}",
                entry.code
            )));
        }
    }

    Ok(())
}

fn validation_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    Box::new(ValidationError::new(message))
}

fn validate_code_policy(entry: &CatalogEntry) -> Result<()> {
    match entry.source {
        DiagnosticSource::Typescript => {
            if !entry.code.starts_with("TS")
                || entry.code[2..].chars().any(|ch| !ch.is_ascii_digit())
            {
                return Err(validation_error(format!(
                    "TypeScript diagnostic code must match TS[0-9]+: {}",
                    entry.code
                )));
            }
        }
        DiagnosticSource::TypescriptRust => {
            if !entry.code.starts_with("typescript-rust::") {
                return Err(validation_error(format!(
                    "custom diagnostic code must start with typescript-rust::: {}",
                    entry.code
                )));
            }
            let suffix = &entry.code["typescript-rust::".len()..];
            if suffix.is_empty()
                || suffix
                    .chars()
                    .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'))
            {
                return Err(validation_error(format!(
                    "custom diagnostic code must be kebab-case: {}",
                    entry.code
                )));
            }
        }
    }

    if entry.support == DiagnosticSupport::CatalogOnly
        && entry.source == DiagnosticSource::TypescriptRust
    {
        return Err(validation_error(format!(
            "custom diagnostics are expected to be emitted in this repository: {}",
            entry.code
        )));
    }

    Ok(())
}

fn placeholder_arity(message: &str) -> Result<usize> {
    let mut highest = None;
    let mut chars = message.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            if ch == '}' {
                return Err(validation_error(format!(
                    "message contains unmatched closing brace: {}",
                    message
                )));
            }
            continue;
        }

        let mut digits = String::new();
        while let Some(next) = chars.peek().copied() {
            if next == '}' {
                chars.next();
                break;
            }

            if next.is_ascii_digit() {
                digits.push(next);
                chars.next();
                continue;
            }

            return Err(validation_error(format!(
                "message contains ambiguous brace sequence: {}",
                message
            )));
        }

        if digits.is_empty() {
            return Err(validation_error(format!(
                "message contains empty placeholder braces: {}",
                message
            )));
        }

        let index = digits.parse::<usize>().map_err(|error| {
            validation_error(format!("failed to parse placeholder index: {error}"))
        })?;
        highest = Some(highest.map_or(index, |current: usize| current.max(index)));
    }

    Ok(highest.map_or(0, |value| value + 1))
}

pub fn generate_rust(entries: &[CatalogEntry]) -> Result<String> {
    validate_catalog(entries)?;

    let mut output = String::new();
    output.push_str("//! Generated diagnostic catalog. Do not edit by hand.\n\n");
    output.push_str(
        "use crate::{\n    Diagnostic, DiagnosticArg, DiagnosticCategory, DiagnosticDescriptor, DiagnosticSource,\n    DiagnosticSupport,\n};\n\n",
    );

    for entry in entries {
        let constant_name = diagnostic_constant_name(entry);
        output.push_str(&format!(
            "pub const {constant_name}: DiagnosticDescriptor = DiagnosticDescriptor {{\n"
        ));
        output.push_str(&format!("    code: {:?},\n", entry.code));
        match entry.source {
            DiagnosticSource::Typescript => {
                let number = entry
                    .code
                    .strip_prefix("TS")
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap();
                output.push_str(&format!("    number: Some({number}),\n"));
                output.push_str("    source: DiagnosticSource::TypeScript,\n");
            }
            DiagnosticSource::TypescriptRust => {
                output.push_str("    number: None,\n");
                output.push_str("    source: DiagnosticSource::TypeScriptRust,\n");
            }
        }
        output.push_str(&format!(
            "    category: DiagnosticCategory::{:?},\n",
            entry.category
        ));
        output.push_str(&format!("    message_template: {:?},\n", entry.message));
        output.push_str(&format!("    argument_count: {},\n", entry.arity));
        output.push_str(&format!(
            "    support: DiagnosticSupport::{:?},\n",
            entry.support
        ));
        output.push_str("};\n\n");
    }

    output.push_str("pub const DIAGNOSTIC_CATALOG: &[DiagnosticDescriptor] = &[\n");
    for entry in entries {
        output.push_str(&format!("    {},\n", diagnostic_constant_name(entry)));
    }
    output.push_str("];\n\n");

    output.push_str("impl Diagnostic {\n");
    for entry in entries {
        output.push_str(&generate_diagnostic_accessor(entry));
    }
    output.push_str("}\n");

    format_rust_source(&output)
}

fn generate_diagnostic_accessor(entry: &CatalogEntry) -> String {
    let function_name = diagnostic_function_name(entry);
    let constant_name = diagnostic_constant_name(entry);

    let mut output = String::new();
    output.push_str("    #[allow(clippy::needless_pass_by_value)]\n");
    output.push_str("    pub fn ");
    output.push_str(&function_name);
    output.push('(');

    for index in 0..entry.arity {
        if index > 0 {
            output.push_str(", ");
        }
        output.push_str(&format!("arg{index}: impl ToString"));
    }

    if entry.arity > 0 {
        output.push_str(", ");
    }
    output.push_str("file_name: impl Into<String>) -> Self {\n");

    if entry.arity == 0 {
        output.push_str(&format!(
            "        Self::from_descriptor(&{constant_name}, Vec::<DiagnosticArg>::new(), file_name)\n"
        ));
    } else {
        output.push_str("        Self::from_descriptor(&");
        output.push_str(&constant_name);
        output.push_str(", vec![");
        for index in 0..entry.arity {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(&format!("DiagnosticArg::from(arg{index}.to_string())"));
        }
        output.push_str("], file_name)\n");
    }

    output.push_str("    }\n\n");
    output
}

pub fn generate_snapshot_toml(entries: &[CatalogEntry]) -> Result<String> {
    validate_catalog(entries)?;

    let mut output = String::new();
    output.push_str("# Pinned verification snapshot for the current diagnostic catalog.\n\n");
    for entry in entries {
        output.push_str("[[diagnostic]]\n");
        output.push_str(&format!("code = {:?}\n", entry.code));
        output.push_str(&format!("category = {:?}\n", category_name(entry.category)));
        output.push_str(&format!("source = {:?}\n", source_name(entry.source)));
        output.push_str(&format!("support = {:?}\n", support_name(entry.support)));
        output.push_str(&format!("message = {:?}\n", entry.message));
        output.push_str(&format!("arity = {}\n\n", entry.arity));
    }

    Ok(output)
}

pub fn diagnostic_function_name(entry: &CatalogEntry) -> String {
    match entry.source {
        DiagnosticSource::Typescript => entry.code.to_ascii_lowercase(),
        DiagnosticSource::TypescriptRust => {
            format!(
                "typescript_rust_{}",
                entry.code["typescript-rust::".len()..].replace('-', "_")
            )
        }
    }
}

pub fn diagnostic_constant_name(entry: &CatalogEntry) -> String {
    match entry.source {
        DiagnosticSource::Typescript => entry.code.to_ascii_uppercase(),
        DiagnosticSource::TypescriptRust => {
            format!(
                "TYPESCRIPT_RUST_{}",
                entry.code["typescript-rust::".len()..]
                    .replace('-', "_")
                    .to_ascii_uppercase()
            )
        }
    }
}

fn category_name(category: DiagnosticCategory) -> &'static str {
    match category {
        DiagnosticCategory::Error => "Error",
        DiagnosticCategory::Warning => "Warning",
        DiagnosticCategory::Suggestion => "Suggestion",
        DiagnosticCategory::Message => "Message",
    }
}

fn source_name(source: DiagnosticSource) -> &'static str {
    match source {
        DiagnosticSource::Typescript => "typescript",
        DiagnosticSource::TypescriptRust => "typescript-rust",
    }
}

fn support_name(support: DiagnosticSupport) -> &'static str {
    match support {
        DiagnosticSupport::CatalogOnly => "catalog-only",
        DiagnosticSupport::Emitted => "emitted",
    }
}

fn format_rust_source(source: &str) -> Result<String> {
    let mut child = Command::new("rustfmt")
        .arg("--edition")
        .arg("2024")
        .arg("--emit")
        .arg("stdout")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(source.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(validation_error(format!(
            "rustfmt failed while formatting generated Rust: {stderr}"
        )));
    }

    Ok(String::from_utf8(output.stdout)?)
}
