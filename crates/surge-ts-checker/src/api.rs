use surge_ts_diagnostics::Diagnostic;

use crate::context::{CheckerOptions, DiagnosticProfile};
use crate::driver::check_source_with_options;
use crate::program::{ProgramCheckResult, SourceFileInput, check_program_with_stats_and_jobs};

/// Outcome of a check: the emitted diagnostics plus the tsc-compatibility stats.
pub type CheckResult = ProgramCheckResult;

/// Entry point for embedding the type checker.
///
/// ```
/// use surge_ts_checker::{Checker, SourceFileInput};
///
/// let result = Checker::new()
///     .no_implicit_any(true)
///     .check(vec![SourceFileInput {
///         file_name: "index.ts".to_string(),
///         source_text: "const x: number = 1;".to_string(),
///     }]);
/// assert!(result.diagnostics.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct Checker {
    options: CheckerOptions,
    jobs: usize,
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    pub fn new() -> Self {
        Self {
            options: CheckerOptions::default(),
            jobs: 1,
        }
    }

    /// Replace the full option set in one call.
    pub fn options(mut self, options: CheckerOptions) -> Self {
        self.options = options;
        self
    }

    /// Borrow the options for fine-grained mutation not covered by a builder method.
    pub fn options_mut(&mut self) -> &mut CheckerOptions {
        &mut self.options
    }

    /// Number of worker threads for multi-file checks. `0` requests automatic
    /// per-phase sizing; other values are used literally.
    ///
    /// The former `.max(1)` clamp silently rewrote the automatic sentinel (`0`,
    /// the CLI's `--jobs auto`) into a forced-serial `1`, leaving the automatic
    /// branches in `resolve_parse_worker_count`/`resolve_worker_count` dead.
    pub fn jobs(mut self, jobs: usize) -> Self {
        self.jobs = jobs;
        self
    }

    pub fn no_implicit_any(mut self, value: bool) -> Self {
        self.options.no_implicit_any = value;
        self
    }

    pub fn no_implicit_returns(mut self, value: bool) -> Self {
        self.options.no_implicit_returns = value;
        self
    }

    pub fn no_fallthrough_cases_in_switch(mut self, value: bool) -> Self {
        self.options.no_fallthrough_cases_in_switch = value;
        self
    }

    pub fn no_implicit_override(mut self, value: bool) -> Self {
        self.options.no_implicit_override = value;
        self
    }

    pub fn no_property_access_from_index_signature(mut self, value: bool) -> Self {
        self.options.no_property_access_from_index_signature = value;
        self
    }

    pub fn no_unused_locals(mut self, value: bool) -> Self {
        self.options.no_unused_locals = value;
        self
    }

    pub fn no_unused_parameters(mut self, value: bool) -> Self {
        self.options.no_unused_parameters = value;
        self
    }

    pub fn no_lib(mut self, value: bool) -> Self {
        self.options.no_lib = value;
        self
    }

    pub fn skip_lib_check(mut self, value: bool) -> Self {
        self.options.skip_lib_check = value;
        self
    }

    /// Suppress non-relative (package) missing-module diagnostics, including the
    /// side-effect `TS2882` form. Relative missing modules and resolved package
    /// declaration errors are unaffected.
    pub fn stub_external_modules(mut self, value: bool) -> Self {
        self.options.stub_external_modules = value;
        self
    }

    /// Effective `@types`-style package names included in the program. Drives the
    /// node-builtin / ambient-global gate (see [`CheckerOptions::types`]).
    pub fn types(mut self, types: Vec<String>) -> Self {
        self.options.types = types;
        self
    }

    /// Pre-resolved module specifiers (specifier → resolved file name). Project
    /// mode populates this from package/`paths` resolution; embedders checking
    /// in-memory programs can supply their own map.
    pub fn resolved_modules(
        mut self,
        resolved_modules: std::collections::HashMap<String, String>,
    ) -> Self {
        self.options.resolved_modules = resolved_modules.into_iter().collect();
        self
    }

    pub fn diagnostic_profile(mut self, profile: DiagnosticProfile) -> Self {
        self.options.diagnostic_profile = profile;
        self
    }

    /// Check a multi-file program, returning diagnostics and compatibility stats.
    pub fn check(self, files: Vec<SourceFileInput>) -> CheckResult {
        check_program_with_stats_and_jobs(files, self.options, self.jobs)
    }

    /// Check a single in-memory source file.
    pub fn check_source(self, source_text: &str, file_name: &str) -> Vec<Diagnostic> {
        check_source_with_options(source_text, file_name, self.options)
    }
}
