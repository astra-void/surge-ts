use std::{path::PathBuf, process::Command};

use serde_json::Value;

fn run_cli(args: &[&str]) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_surge"))
        .args(args)
        .output()
        .unwrap();

    assert!(
        matches!(output.status.code(), Some(0) | Some(2)),
        "surge exited with unexpected status {:?}",
        output.status.code()
    );
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn compat_project_root(name: &str) -> PathBuf {
    workspace_root().join("tests/compat-projects").join(name)
}

#[test]
fn compat_project_generics_basic_valid_subset_passes() {
    let project = compat_project_root("generics-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn compat_project_generics_basic_report_stable() {
    let project = compat_project_root("generics-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Compatibility report"));
    assert!(stdout.contains("Files loaded: 3"));
    assert!(stdout.contains("Diagnostics: 0"));
}

#[test]
fn compat_report_generics_reduces_parser_errors_or_pins_remaining() {
    let project = compat_project_root("generics-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("\"filesLoaded\""));
}

#[test]
fn compat_report_generics_json_stable() {
    let project = compat_project_root("generics-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert!(stderr.is_empty());

    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["filesLoaded"], Value::from(3));
    assert_eq!(parsed["diagnosticsTotal"], Value::from(0));
    assert_eq!(parsed["byCode"], serde_json::json!([]));
}

#[test]
fn compat_report_generics_counts_by_code_stable() {
    let project = compat_project_root("generics-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert!(stderr.is_empty());

    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["byCode"], serde_json::json!([]));
    assert_eq!(parsed["byFile"], serde_json::json!([]));
}

#[test]
fn compat_report_generics_unsupported_file_still_parser_safe_or_pinned() {
    let project = compat_project_root("generics-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Files loaded: 3"));
    assert!(stdout.contains("Diagnostics: 0"));
}
