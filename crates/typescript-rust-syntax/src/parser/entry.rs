use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::ParsedSource;

pub fn parse_source(source_text: &str, file_name: &str) -> ParsedSource {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser = Parser::new(&allocator, source_text, source_type);
    let parsed = parser.parse();

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

    ParsedSource {
        file_name: file_name.to_string(),
        statements,
        parser_errors,
        is_module,
    }
}
