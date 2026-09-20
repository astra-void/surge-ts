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
            crate::ParserError { code, message: error.to_string(), span }
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

    // Grammar findings are reported for hand-written TypeScript only: surge
    // suppresses every declaration-file diagnostic, and a `.js` file is not
    // type-checked the way `checkJs` would need.
    let (grammar_diagnostics, parenthesized_expressions) = if collects_grammar_diagnostics(file_name) {
        super::grammar::collect_grammar_diagnostics(&parsed.program)
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

fn collects_grammar_diagnostics(file_name: &str) -> bool {
    if file_name.ends_with(".d.ts")
        || file_name.ends_with(".d.mts")
        || file_name.ends_with(".d.cts")
    {
        return false;
    }
    [".ts", ".tsx", ".mts", ".cts"]
        .iter()
        .any(|extension| file_name.ends_with(extension))
}
