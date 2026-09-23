use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::{ParsedSource, ParsedType};

/// A reusable parsing context that owns one oxc arena allocator and amortizes
/// its chunk allocations across many `parse` calls.
///
/// Safety model: [`ParsedSource`] is fully owned (`String`s and `Vec`s, no
/// lifetimes), so nothing returned by `parse` can reference the arena. The
/// arena is reset at the *start* of each parse, which also guarantees a valid
/// worker state after an earlier parse panicked or errored. A worker must not
/// be shared between threads (the arena is not thread-safe); create one worker
/// per parsing thread.
pub struct ParserWorker {
    allocator: Allocator,
}

impl ParserWorker {
    pub fn new() -> Self {
        Self {
            allocator: Allocator::default(),
        }
    }

    pub fn parse(&mut self, source_text: &str, file_name: &str) -> ParsedSource {
        // Mass-deallocates the previous file's AST (no Drop impls run; oxc AST
        // nodes are Drop-free by design) and keeps the largest chunk for reuse.
        self.allocator.reset();
        parse_source_in(&self.allocator, source_text, file_name)
    }
}

impl Default for ParserWorker {
    fn default() -> Self {
        Self::new()
    }
}

/// One-shot parse with a fresh arena. Prefer [`ParserWorker`] when parsing many
/// files in a loop.
pub fn parse_source(source_text: &str, file_name: &str) -> ParsedSource {
    let allocator = Allocator::default();
    parse_source_in(&allocator, source_text, file_name)
}

/// Parse `source_text` into fully owned surge structures. Every borrow of
/// `allocator` ends inside this function: the returned [`ParsedSource`] holds
/// no references, pointers, or arena-backed strings, which is what makes
/// resetting the allocator between calls sound.
/// The TypeScript number of a failure oxc reports without one, where tsc
/// reports the same construct as a grammar error: an invalid write target —
/// TS2364 for an assignment, TS2357 for `++`/`--`, TS2779/TS2777 when it is an
/// optional chain — and a misplaced rest parameter (TS1014) or rest element
/// (TS2462). oxc stops parsing the file at these, so they are all that is
/// left to report for it.
fn classify_uncoded_parser_error(
    message: &str,
    span: crate::TextSpan,
    source_text: &str,
) -> Option<(u32, crate::TextSpan)> {
    let text = source_text.get(span.start..span.end)?;
    match message {
        "Cannot assign to this expression" => {
            // tsc's target node keeps the parentheses oxc's label drops.
            let mut span = span;
            loop {
                let before = source_text[..span.start].trim_end();
                let after = source_text[span.end..].trim_start();
                if !(before.ends_with('(') && after.starts_with(')')) {
                    break;
                }
                span = crate::TextSpan {
                    start: before.len() - 1,
                    end: source_text.len() - after.len() + 1,
                };
            }
            let before = source_text[..span.start].trim_end();
            let after = source_text[span.end..].trim_start();
            let is_update = before.ends_with("++")
                || before.ends_with("--")
                || after.starts_with("++")
                || after.starts_with("--");
            let optional = text.contains("?.");
            let code = match (is_update, optional) {
                (false, false) => 2364,
                (false, true) => 2779,
                (true, false) => 2357,
                (true, true) => 2777,
            };
            Some((code, span))
        }
        "A rest parameter must be last in a parameter list" => Some((1014, span)),
        // tsc reports the rest element at its name, past the `...`.
        "A rest element must be last in a destructuring pattern" => {
            let name = text.strip_prefix("...").map_or(text, str::trim_start);
            let start = span.end - name.len();
            Some((2462, crate::TextSpan { start, end: span.end }))
        }
        _ => None,
    }
}

/// Where tsc anchors a failure oxc labels elsewhere: the name after a
/// `const` class member modifier (TS1248), the `<` of an instantiation
/// expression (TS1477), the second of the `u`/`v` flags (TS1502), and the
/// first keyword of an ambient `using` or `await using` (TS1545/TS1546).
fn tsc_anchor_for_parser_error(
    code: Option<u32>,
    span: crate::TextSpan,
    source_text: &str,
) -> crate::TextSpan {
    let text = |start: usize, end: usize| source_text.get(start..end).unwrap_or("");
    let at = |start: usize, len: usize| crate::TextSpan { start, end: start + len };
    match code {
        Some(1248) => {
            let rest = text(span.end, source_text.len());
            let skipped = rest.len() - rest.trim_start().len();
            let start = span.end + skipped;
            let len = text(start, source_text.len())
                .find(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '$'))
                .unwrap_or(0);
            at(start, len)
        }
        Some(1477) => match text(span.start, span.end).find('<') {
            Some(offset) => at(span.start + offset, 1),
            None => span,
        },
        Some(1502) if span.end > span.start => at(span.end - 1, 1),
        Some(1545) => match text(0, span.start).rfind("using") {
            Some(start) => at(start, 5),
            None => span,
        },
        Some(1546) => match text(0, span.start).rfind("await") {
            Some(start) => at(start, 5),
            None => span,
        },
        _ => span,
    }
}

fn parse_source_in(allocator: &Allocator, source_text: &str, file_name: &str) -> ParsedSource {
    // A `.json` file holds a value, not a program. Handing it to the TypeScript
    // parser produces nothing usable (and a pile of syntax errors), so it takes
    // its own path and carries only the value's type.
    if super::is_json_file_name(file_name) {
        return ParsedSource {
            file_name: file_name.to_string(),
            statements: Vec::new(),
            parser_errors: Vec::new(),
            is_module: true,
            reference_type_directives: Vec::new(),
            module_reads: Vec::new(),
            definite_writes: Vec::new(),
            let_assignments: Vec::new(),
            suppressed_ranges: Vec::new(),
            import_call_specifiers: Vec::new(),
            grammar_diagnostics: Vec::new(),
            parenthesized_expressions: Vec::new(),
            // A `.json` file that does not parse still *is* a JSON module —
            // reporting its importer as unresolved would be a worse answer than
            // an unmodelled value, and surge does not report JSON syntax errors.
            json_module_type: Some(
                super::parse_json_module_type(source_text).unwrap_or(ParsedType::Unknown),
            ),
        };
    }

    let source_type = SourceType::from_path(file_name).unwrap_or_else(|_| SourceType::ts());
    let parser = Parser::new(allocator, source_text, source_type);
    let parsed = parser.parse();

    let reference_type_directives = super::extract_reference_type_directives(source_text);
    let suppressed_ranges =
        super::suppressions::collect_suppressed_ranges(source_text, &parsed.program.comments);

    let collect_statements = || -> Vec<crate::ParsedStatement> {
        let mut statements: Vec<crate::ParsedStatement> = parsed
            .program
            .body
            .iter()
            .filter_map(super::parse_statement)
            .flatten()
            .collect();
        super::enums::merge_lowered_enum_declarations(&mut statements);
        statements
    };

    // Declaration files never participate in noUnusedLocals, and `declare`
    // functions carry no body to index, so skip the read walk for them entirely
    // (it would otherwise run over every dependency `.d.ts`). The conversion
    // still asks each body for its reads; without an index those calls fall back
    // to walking the body, which for a `.d.ts` is nothing.
    let (module_reads, statements) = if file_name.ends_with(".d.ts") {
        (Vec::new(), collect_statements())
    } else {
        super::spans::with_lowering_source(source_text, || {
            super::reads::with_body_read_index(&parsed.program, collect_statements)
        })
    };

    let parser_errors = parsed
        .errors
        .into_iter()
        .map(|error| {
            let code = error
                .code
                .scope
                .as_deref()
                .filter(|scope| *scope == "TS")
                .and(error.code.number.as_deref())
                .and_then(|number| number.parse::<u32>().ok());
            let span = error
                .labels
                .as_ref()
                .and_then(|labels| labels.first())
                .map(|label| crate::TextSpan {
                    start: label.offset(),
                    end: label.offset() + label.len(),
                });
            let (code, span) = match (code, span) {
                (None, Some(span)) => match classify_uncoded_parser_error(&error.to_string(), span, source_text) {
                    Some((code, span)) => (Some(code), Some(span)),
                    None => (None, Some(span)),
                },
                other => other,
            };
            let span = span.map(|span| tsc_anchor_for_parser_error(code, span, source_text));
            let span_text = span
                .and_then(|span| source_text.get(span.start..span.end))
                .map(str::to_string);
            crate::ParserError { code, message: error.to_string(), span, span_text }
        })
        .collect();

    let is_module = parsed.program.source_type.is_module()
        || statements.iter().any(|statement| {
            matches!(
                statement,
                crate::ParsedStatement::ImportDeclaration(_)
                    | crate::ParsedStatement::ExportDeclaration(_)
            )
        });

    let import_call_specifiers =
        super::import_calls::collect_import_call_specifiers(&parsed.program, source_text);

    // Grammar findings are reported for hand-written TypeScript only: a
    // declaration file gets just the top-level `declare` requirement (which
    // `skipLibCheck` then suppresses), and a `.js` file is not type-checked
    // the way `checkJs` would need.
    let (grammar_diagnostics, parenthesized_expressions) = if collects_grammar_diagnostics(file_name) {
        super::grammar::collect_grammar_diagnostics(&parsed.program)
    } else if is_declaration_file_name(file_name) {
        let mut diagnostics = Vec::new();
        super::grammar_modifiers::collect_declaration_file_diagnostics(&parsed.program, &mut diagnostics);
        (diagnostics, Vec::new())
    } else {
        (Vec::new(), Vec::new())
    };

    let let_assignments =
        super::let_assignments::collect_let_assignments(&parsed.program, source_text);
    ParsedSource {
        file_name: file_name.to_string(),
        statements,
        parser_errors,
        is_module,
        reference_type_directives,
        module_reads,
        definite_writes: super::writes::collect_definite_writes(&parsed.program),
        let_assignments,
        suppressed_ranges,
        import_call_specifiers,
        grammar_diagnostics,
        parenthesized_expressions,
        json_module_type: None,
    }
}

fn is_declaration_file_name(file_name: &str) -> bool {
    file_name.ends_with(".d.ts") || file_name.ends_with(".d.mts") || file_name.ends_with(".d.cts")
}

fn collects_grammar_diagnostics(file_name: &str) -> bool {
    if is_declaration_file_name(file_name) {
        return false;
    }
    [".ts", ".tsx", ".mts", ".cts"]
        .iter()
        .any(|extension| file_name.ends_with(extension))
}
