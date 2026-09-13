use surge_ts_checker::{
    CheckerOptions, DiagnosticProfile, SourceFileInput, check_program, check_program_with_options,
};

mod dependency_declarations;
mod cross_file_scripts;
mod compiler_flags;
mod assorted_checks;
mod modules_imports;
mod modules_reexports;
mod ambient_modules;
mod declaration_files;
mod generics;
mod caches_and_parallel;
mod trpc_burndown;

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn file_names(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.file_name.clone())
        .collect()
}

fn program(files: &[(&str, &str)]) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_program(
        files
            .iter()
            .map(|(file_name, source_text)| SourceFileInput {
                file_name: (*file_name).to_string(),
                source_text: (*source_text).to_string(),
            })
            .collect(),
    )
}

fn program_with_options(
    files: &[(&str, &str)],
    options: CheckerOptions,
) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_program_with_options(
        files
            .iter()
            .map(|(file_name, source_text)| SourceFileInput {
                file_name: (*file_name).to_string(),
                source_text: (*source_text).to_string(),
            })
            .collect(),
        options,
    )
}

fn native_program(files: &[(&str, &str)]) -> Vec<surge_ts_diagnostics::Diagnostic> {
    let mut options = CheckerOptions::default();
    options.diagnostic_profile = DiagnosticProfile::Native;
    program_with_options(files, options)
}

fn dependency_program(
    declaration_file: &str,
    declaration_source: &str,
    consumer_source: &str,
) -> Vec<surge_ts_diagnostics::Diagnostic> {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), declaration_file.to_string());
    program_with_options(
        &[
            (declaration_file, declaration_source),
            ("src/index.ts", consumer_source),
        ],
        options,
    )
}
