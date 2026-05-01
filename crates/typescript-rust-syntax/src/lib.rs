//! Parsed TypeScript syntax and parser entrypoint.

mod ast;
mod parser;

pub use ast::*;
pub use parser::parse_source;

#[cfg(test)]
mod tests;
