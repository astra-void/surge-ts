use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::ParsedSource;

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
    let source_type = SourceType::from_path(file_name).unwrap_or_else(|_| SourceType::ts());
    let parser = Parser::new(allocator, source_text, source_type);
    let parsed = parser.parse();

    let reference_type_directives = super::extract_reference_type_directives(source_text);

    let statements: Vec<crate::ParsedStatement> = parsed
        .program
        .body
        .iter()
        .filter_map(super::parse_statement)
        .flatten()
        .collect();

    let parser_errors = parsed
        .errors
        .into_iter()
        .map(|error| error.to_string())
        .collect();

    let is_module = parsed.program.source_type.is_module()
        || statements.iter().any(|statement| {
            matches!(
                statement,
                crate::ParsedStatement::ImportDeclaration(_)
                    | crate::ParsedStatement::ExportDeclaration(_)
            )
        });

    // Declaration files never participate in noUnusedLocals, so skip the
    // module-wide read walk for them (avoids the cost on every dependency `.d.ts`).
    let module_reads = if file_name.ends_with(".d.ts") {
        Vec::new()
    } else {
        super::reads::collect_program_reads(&parsed.program)
    };

    ParsedSource {
        file_name: file_name.to_string(),
        statements,
        parser_errors,
        is_module,
        reference_type_directives,
        module_reads,
    }
}
