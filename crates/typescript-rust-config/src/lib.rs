//! tsconfig loading and normalization.

mod diagnostics;
mod extends;
mod files;
mod model;
mod normalize;
mod options;
mod parse;
mod paths;

pub use diagnostics::*;
pub use model::*;
pub use options::*;
pub use parse::load_tsconfig;

#[cfg(test)]
mod tests;
