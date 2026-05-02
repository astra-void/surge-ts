use std::{fs, path::PathBuf, process::Command};

use serde_json::Value;

fn run_cli(args: &[&str]) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .args(args)
        .output()
        .unwrap();

    assert!(output.status.success());
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
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

#[test]
fn cli_diagnostics_json_single_file() {
    let root = temp_dir("json-single-file");
    let file = root.join("index.ts");
    fs::write(&file, "let value: string = 123;").unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--format", "json", file.as_str()]);

    assert!(stderr.is_empty());

    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 1);
    let diagnostic = &parsed["diagnostics"][0];
    assert_eq!(diagnostic["code"], Value::String("TS2322".to_string()));
    assert!(
        diagnostic["fileName"]
            .as_str()
            .unwrap()
            .ends_with("index.ts")
    );
    assert!(!diagnostic["message"].as_str().unwrap().is_empty());
    assert!(diagnostic["span"]["start"].is_number());
    assert!(diagnostic["span"]["end"].is_number());
    assert!(diagnostic["line"].as_u64().unwrap() >= 1);
    assert!(diagnostic["column"].as_u64().unwrap() >= 1);
}

#[test]
fn cli_diagnostics_json_project_normalizes_paths() {
    let root = temp_dir("json-project");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "let value: string = 123;");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--format", "json"]);

    assert!(stderr.is_empty());

    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 1);
    let diagnostic = &parsed["diagnostics"][0];
    assert_eq!(diagnostic["code"], Value::String("TS2322".to_string()));
    assert_eq!(
        diagnostic["fileName"],
        Value::String("src/index.ts".to_string())
    );
    assert!(diagnostic["span"]["start"].is_number());
    assert!(diagnostic["span"]["end"].is_number());
    assert!(diagnostic["line"].as_u64().unwrap() >= 1);
    assert!(diagnostic["column"].as_u64().unwrap() >= 1);
}

#[test]
fn cli_diagnostics_text_output_unchanged_without_format() {
    let root = temp_dir("json-text-output");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "let value: string = 123;");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2322"));
    assert!(stdout.contains("src/index.ts"));
    assert!(!stdout.trim_start().starts_with('{'));
}
