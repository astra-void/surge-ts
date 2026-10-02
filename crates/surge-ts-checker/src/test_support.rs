use std::process::Command;

const CHILD_MARKER: &str = "SURGE_TEST_OWN_PROCESS";

/// Re-runs one test alone in a fresh copy of the test binary and reports
/// whether the caller is the parent. `cargo test` runs tests as threads of one
/// process, so a test that sets environment gates or reads the process-global
/// program counters must not share that process with other checks.
///
/// Call as `if run_in_own_process(module_path!(), "test_fn") { return; }`.
pub(crate) fn run_in_own_process(module_path: &str, test_name: &str) -> bool {
    if std::env::var_os(CHILD_MARKER).is_some() {
        return false;
    }
    let module = module_path
        .split_once("::")
        .map_or("", |(_crate_name, rest)| rest);
    let filter = if module.is_empty() {
        test_name.to_string()
    } else {
        format!("{module}::{test_name}")
    };
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args([filter.as_str(), "--exact", "--test-threads=1", "--nocapture"])
        .env(CHILD_MARKER, "1")
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
