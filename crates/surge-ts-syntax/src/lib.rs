//! Parsed TypeScript syntax and parser entrypoint.

mod ast;
pub mod clone_census;
mod parser;

pub use ast::*;
pub use parser::{
    ParserWorker, extract_check_directive, extract_reference_path_directives,
    extract_reference_type_directives, is_declaration_file_name, is_json_file_name,
    js_number_to_string, jsx_entity_root, parse_json_module_type, parse_source,
};

#[cfg(test)]
mod tests;
