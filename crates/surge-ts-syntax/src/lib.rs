//! Parsed TypeScript syntax and parser entrypoint.

mod ast;
mod parser;

pub use ast::*;
pub use parser::{
    ParserWorker, extract_reference_path_directives, extract_reference_type_directives,
    parse_source,
};

#[cfg(test)]
mod tests;
