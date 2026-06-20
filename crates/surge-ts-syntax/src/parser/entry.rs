use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::ParsedSource;

pub fn parse_source(source_text: &str, file_name: &str) -> ParsedSource {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(file_name).unwrap_or_else(|_| SourceType::ts());
    let parser = Parser::new(&allocator, source_text, source_type);
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
