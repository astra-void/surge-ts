//! Type checker for TypeScript, embeddable as a library.
//!
//! The stable entry point is [`Checker`], a builder that takes source files and
//! returns a [`CheckResult`] (diagnostics plus tsc-compatibility stats):
//!
//! ```
//! use surge_ts_checker::{Checker, SourceFileInput};
//!
//! let result = Checker::new().check(vec![SourceFileInput {
//!     file_name: "index.ts".to_string(),
//!     source_text: "let x: string = 1;".to_string(),
//! }]);
//! assert!(!result.diagnostics.is_empty());
//! ```
//!
//! Lower-level building blocks (default-lib loading and resolution) live in
//! [`lowlevel`] and are not covered by the stable-API guarantees.

mod api;
mod arena;
mod checks;
mod context;
mod default_lib;
mod driver;
mod flow;
mod infer;
mod metrics;
mod modules;
mod paths;
mod program;
mod spans;
mod speculative;
mod symbols;

pub use api::{CheckResult, Checker};
pub use context::{CheckerOptions, CompatibilityStats, DiagnosticProfile, FileKind};
pub use program::{ProgramCheckResult, SourceFileInput};

/// Diagnostic types are re-exported so embedders can read [`Checker`] output
/// (code, message, span, severity) without depending on `surge-ts-diagnostics`
/// directly.
pub use surge_ts_diagnostics::{Diagnostic, DiagnosticCategory, DiagnosticCode, TextSpan};

#[doc(hidden)]
pub use driver::{check_source, check_source_with_options};
#[doc(hidden)]
pub use program::{
    check_program, check_program_with_options, check_program_with_stats,
    check_program_with_stats_and_jobs,
};

/// Lower-level building blocks for custom checking pipelines: default-lib
/// loading, resolution, and the seed catalog. Not part of the stable API
/// surface — these shapes can change without a major-version bump.
pub mod lowlevel {
    pub use crate::default_lib::{
        DefaultLibIoStats, DefaultLibLoad, DefaultLibRequest, PhysicalLibResolution,
        default_full_lib_seed_for_target, load_default_lib_inputs,
        load_generated_default_lib_inputs, resolve_physical_default_libs,
    };
    pub use crate::metrics::record_loader_rss_stage;

    /// Centralized relative-path candidate generation shared by the loader's
    /// import-graph/`paths` resolution and the checker's module binding, so the
    /// extension-substitution matrix cannot drift between layers.
    pub mod resolution_candidates {
        pub use crate::modules::candidates::{
            RelativeSpecifierShape, classify_relative_specifier, directory_index_candidates,
            extensionless_candidates, mapped_target_candidates, relative_import_candidates,
            strip_extension,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_profile_is_public_api() {
        let options = CheckerOptions {
            diagnostic_profile: DiagnosticProfile::Native,
            types: Vec::new(),
            ..CheckerOptions::default()
        };
        assert_eq!(options.diagnostic_profile, DiagnosticProfile::Native);
    }
}
