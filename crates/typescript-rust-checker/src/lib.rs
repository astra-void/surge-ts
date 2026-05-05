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

pub use context::{CheckerOptions, DiagnosticProfile};
pub use driver::{check_source, check_source_with_options};
pub use program::{SourceFileInput, check_program, check_program_with_options};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_profile_is_public_api() {
        let options = CheckerOptions {
            diagnostic_profile: DiagnosticProfile::Native,
            ..CheckerOptions::default()
        };
        assert_eq!(options.diagnostic_profile, DiagnosticProfile::Native);
    }
}
