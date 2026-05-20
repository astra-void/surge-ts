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
pub use paths::{
    absolutize, canonicalize_if_exists, canonicalize_if_exists_string, cycle_key,
    normalize_path_buf, normalize_path_string, resolve_path, resolve_project_path,
};

#[cfg(test)]
mod tests;
