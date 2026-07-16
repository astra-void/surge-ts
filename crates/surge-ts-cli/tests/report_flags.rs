use std::{fs, path::PathBuf, process::Command, process::Output};

use serde_json::Value;

fn run_cli_raw(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surge"))
        .args(args)
        .output()
        .unwrap()
}

fn run_cli(args: &[&str]) -> (Vec<u8>, String) {
    let output = run_cli_raw(args);
    assert!(
        matches!(output.status.code(), Some(0) | Some(2)),
        "surge exited with unexpected status {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.stdout, String::from_utf8(output.stderr).unwrap())
}

fn temp_dir(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_file(root: &PathBuf, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn fixture_project(prefix: &str) -> (PathBuf, String) {
    let root = temp_dir(prefix);
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "let value: string = 123;");
    let project = root.join("tsconfig.json").to_string_lossy().into_owned();
    (root, project)
}

#[test]
fn default_concise_output_has_no_new_noise() {
    let (_root, project) = fixture_project("report-default-output");
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("TS2322"));
    assert!(!stdout.contains("Extended diagnostics"));
    assert!(!stdout.contains("Memory report"));
}

#[test]
fn extended_diagnostics_renders_stderr_block_and_keeps_stdout_identical() {
    let (_root, project) = fixture_project("report-extended");
    let (plain_stdout, plain_stderr) = run_cli(&["--project", project.as_str()]);
    let (extended_stdout, extended_stderr) =
        run_cli(&["--project", project.as_str(), "--extendedDiagnostics"]);

    assert!(plain_stderr.is_empty());
    assert_eq!(
        plain_stdout, extended_stdout,
        "stdout bytes must not change"
    );

    assert!(extended_stderr.contains("Extended diagnostics:"));
    for label in [
        "files:",
        "source files:",
        "dependency declaration files:",
        "default lib files:",
        "diagnostics:",
        "jobs:",
        "allocator:",
        "checking:",
        "total:",
        "peak physical footprint:",
        "finish physical footprint:",
        "peak rss:",
    ] {
        assert!(
            extended_stderr.contains(label),
            "missing {label:?} in:\n{extended_stderr}"
        );
    }
    assert!(extended_stderr.contains("diagnostics:") && extended_stderr.contains(" 1"));
}

#[test]
fn memory_report_renders_stderr_block_and_keeps_stdout_identical() {
    let (_root, project) = fixture_project("report-memory");
    let (plain_stdout, _) = run_cli(&["--project", project.as_str()]);
    let (memory_stdout, memory_stderr) =
        run_cli(&["--project", project.as_str(), "--memoryReport"]);

    assert_eq!(plain_stdout, memory_stdout, "stdout bytes must not change");
    assert!(memory_stderr.contains("Memory report:"));
    assert!(memory_stderr.contains("peak physical footprint:"));
    assert!(memory_stderr.contains("finish physical footprint:"));
    assert!(memory_stderr.contains("peak rss:"));
    assert!(!memory_stderr.contains("Extended diagnostics"));
}

#[test]
fn report_json_writes_versioned_report_and_keeps_stdout_identical() {
    let (root, project) = fixture_project("report-json");
    let report_path = root.join("report.json");
    let report_path_str = report_path.to_string_lossy().into_owned();

    let (plain_stdout, _) = run_cli(&["--project", project.as_str()]);
    let (json_stdout, _) = run_cli(&[
        "--project",
        project.as_str(),
        "--reportJson",
        report_path_str.as_str(),
    ]);

    assert_eq!(plain_stdout, json_stdout, "stdout bytes must not change");

    let raw = fs::read_to_string(&report_path).unwrap();
    let parsed: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["schemaVersion"], Value::from(1));

    let summary = parsed["summary"].as_object().unwrap();
    for key in [
        "files",
        "sourceFiles",
        "dependencyDeclarationFiles",
        "defaultLibFiles",
        "diagnostics",
        "wallTimeMs",
        "jobs",
        "allocator",
    ] {
        assert!(summary.contains_key(key), "summary missing {key}");
    }
    let phases = parsed["phases"].as_object().unwrap();
    for key in [
        "configProjectLoadingMs",
        "fileDiscoveryMs",
        "defaultLibLoadingMs",
        "packageDeclarationDiscoveryMs",
        "importGraphExpansionMs",
        "pathMappingResolutionMs",
        "checkingMs",
        "diagnosticRenderingMs",
        "totalMs",
    ] {
        assert!(phases.contains_key(key), "phases missing {key}");
        assert!(phases[key].is_number(), "{key} must be a number");
    }
    let memory = parsed["memory"].as_object().unwrap();
    for key in ["peakPhysicalBytes", "finishPhysicalBytes", "peakRssBytes"] {
        assert!(memory.contains_key(key), "memory missing {key}");
        let value = &memory[key];
        assert!(
            value.is_null() || value.as_u64().is_some_and(|bytes| bytes > 0),
            "{key} must be null or a positive byte count, got {value}"
        );
    }

    assert_eq!(parsed["summary"]["diagnostics"], Value::from(1));
    let files = summary["files"].as_u64().unwrap();
    let source_files = summary["sourceFiles"].as_u64().unwrap();
    let dependency = summary["dependencyDeclarationFiles"].as_u64().unwrap();
    let default_lib = summary["defaultLibFiles"].as_u64().unwrap();
    assert_eq!(files, source_files + dependency + default_lib);
    assert!(source_files >= 1);
    assert!(summary["wallTimeMs"].as_f64().unwrap() > 0.0);
}

fn key_sequence(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                out.push(key.clone());
                key_sequence(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                key_sequence(item, out);
            }
        }
        _ => {}
    }
}

fn zero_numbers(value: &mut Value) {
    match value {
        Value::Number(_) => *value = Value::from(0),
        Value::Object(map) => {
            for (_, child) in map.iter_mut() {
                zero_numbers(child);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                zero_numbers(item);
            }
        }
        _ => {}
    }
}

#[test]
fn report_json_is_deterministic_across_runs_modulo_volatile_values() {
    let (root, project) = fixture_project("report-json-deterministic");
    let first_path = root.join("report-a.json");
    let second_path = root.join("report-b.json");

    for path in [&first_path, &second_path] {
        run_cli(&[
            "--project",
            project.as_str(),
            "--reportJson",
            path.to_string_lossy().as_ref(),
        ]);
    }

    let first: Value = serde_json::from_str(&fs::read_to_string(&first_path).unwrap()).unwrap();
    let second: Value = serde_json::from_str(&fs::read_to_string(&second_path).unwrap()).unwrap();

    let mut first_keys = Vec::new();
    let mut second_keys = Vec::new();
    key_sequence(&first, &mut first_keys);
    key_sequence(&second, &mut second_keys);
    assert_eq!(first_keys, second_keys, "key sequence must be identical");

    let (mut first, mut second) = (first, second);
    zero_numbers(&mut first);
    zero_numbers(&mut second);
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap(),
        "reports must be byte-identical once volatile numeric values are zeroed"
    );
}

#[test]
fn report_json_jobs_metadata_reflects_jobs_flag() {
    let (root, project) = fixture_project("report-json-jobs");

    let auto_path = root.join("report-auto.json");
    run_cli(&[
        "--project",
        project.as_str(),
        "--reportJson",
        auto_path.to_string_lossy().as_ref(),
    ]);
    let auto: Value = serde_json::from_str(&fs::read_to_string(&auto_path).unwrap()).unwrap();
    assert_eq!(auto["summary"]["jobs"], Value::String("auto".to_string()));

    let serial_path = root.join("report-serial.json");
    run_cli(&[
        "--project",
        project.as_str(),
        "--jobs",
        "1",
        "--reportJson",
        serial_path.to_string_lossy().as_ref(),
    ]);
    let serial: Value = serde_json::from_str(&fs::read_to_string(&serial_path).unwrap()).unwrap();
    assert_eq!(serial["summary"]["jobs"], Value::from(1));
}

#[test]
fn report_json_counts_dependency_declarations() {
    let root = temp_dir("report-json-dependency");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "node_modules/dep/package.json",
        r#"{ "name": "dep", "types": "index.d.ts" }"#,
    );
    write_file(
        &root,
        "node_modules/dep/index.d.ts",
        "export declare const answer: number;\n",
    );
    write_file(
        &root,
        "src/index.ts",
        "import { answer } from \"dep\";\nlet value: string = answer;\n",
    );

    let project = root.join("tsconfig.json").to_string_lossy().into_owned();
    let report_path = root.join("report.json");
    run_cli(&[
        "--project",
        project.as_str(),
        "--reportJson",
        report_path.to_string_lossy().as_ref(),
    ]);

    let parsed: Value = serde_json::from_str(&fs::read_to_string(&report_path).unwrap()).unwrap();
    assert_eq!(
        parsed["summary"]["dependencyDeclarationFiles"],
        Value::from(1)
    );
    assert_eq!(parsed["summary"]["sourceFiles"], Value::from(1));
}

#[test]
fn report_flags_compose_with_format_json() {
    let (root, project) = fixture_project("report-format-json");
    let report_path = root.join("report.json");

    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--extendedDiagnostics",
        "--reportJson",
        report_path.to_string_lossy().as_ref(),
    ]);

    let stdout = String::from_utf8(stdout).unwrap();
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 1);
    assert!(stderr.contains("Extended diagnostics:"));
    assert!(report_path.exists());
}

#[test]
fn report_flags_require_project() {
    let root = temp_dir("report-requires-project");
    let file = root.join("index.ts");
    fs::write(&file, "let value: string = 123;").unwrap();
    let file = file.to_string_lossy().into_owned();

    for (flag, extra) in [
        ("--extendedDiagnostics", None),
        ("--memoryReport", None),
        ("--reportJson", Some("out.json")),
    ] {
        let mut args = vec!["--ignoreConfig", flag];
        if let Some(value) = extra {
            args.push(value);
        }
        args.push(file.as_str());
        let output = run_cli_raw(&args);
        assert!(
            !output.status.success(),
            "{flag} without --project must fail"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(&format!("{flag} requires --project")),
            "unexpected stderr for {flag}: {stderr}"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn report_flags_conflict_with_show_config() {
    let (root, project) = fixture_project("report-show-config-conflict");
    let report_path = root.join("report.json");

    let output = run_cli_raw(&[
        "--project",
        project.as_str(),
        "--showConfig",
        "--reportJson",
        report_path.to_string_lossy().as_ref(),
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--reportJson cannot be used with --showConfig"),
        "unexpected stderr: {stderr}"
    );
    assert!(!report_path.exists());
}

#[test]
fn report_json_write_failure_is_a_clear_error() {
    let (root, project) = fixture_project("report-json-write-failure");
    let missing_dir = root.join("missing-dir").join("report.json");

    let output = run_cli_raw(&[
        "--project",
        project.as_str(),
        "--reportJson",
        missing_dir.to_string_lossy().as_ref(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("failed to write"),
        "unexpected stderr: {stderr}"
    );
}
