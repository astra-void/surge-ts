use std::process::Command;

use surge_ts_checker::{CheckerOptions, SourceFileInput, check_program, check_program_with_options};

mod caches_and_parallel;
mod compiler_flags;
mod cross_file_scripts;

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

const OWN_PROCESS_MARKER: &str = "SURGE_TEST_OWN_PROCESS";

/// Re-runs one test alone in a fresh copy of this test binary and reports
/// whether the caller is the parent. `cargo test` runs tests as threads of one
/// process, so a test that sets an environment override must not share that
/// process with other checks.
///
/// Call as `if run_in_own_process(module_path!(), "test_fn") { return; }`.
fn run_in_own_process(module_path: &str, test_name: &str) -> bool {
    if std::env::var_os(OWN_PROCESS_MARKER).is_some() {
        return false;
    }
    let module = module_path
        .split_once("::")
        .map_or("", |(_binary_name, rest)| rest);
    let filter = if module.is_empty() {
        test_name.to_string()
    } else {
        format!("{module}::{test_name}")
    };
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args([filter.as_str(), "--exact", "--test-threads=1", "--nocapture"])
        .env(OWN_PROCESS_MARKER, "1")
        .output()
        .expect("spawn isolated test process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "isolated run of {filter} failed ({}):\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    true
}
