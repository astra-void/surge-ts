mod builtins;
mod checks;
mod context;
mod driver;
mod flow;
mod infer;
mod modules;
mod program;
mod spans;
mod symbols;

pub use context::{CheckerOptions, CompatibilityStats, DiagnosticProfile, FileKind};
pub use driver::{check_source, check_source_with_options};
pub use program::{
    ProgramCheckResult, SourceFileInput, check_program, check_program_with_options,
    check_program_with_stats, check_program_with_stats_and_jobs,
};

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
