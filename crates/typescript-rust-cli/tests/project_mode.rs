use std::{fs, path::PathBuf, process::Command, time::SystemTime};

use serde_json::Value;
use typescript_rust_checker::{CheckerOptions, check_source_with_options};
use typescript_rust_config::{TsConfigLoadOptions, load_tsconfig};

fn temp_dir(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
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

#[test]
fn project_mode_maps_strict_to_no_implicit_any() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let project = workspace_root.join("tests/tsconfig/basic/tsconfig.json");

    let loaded = load_tsconfig(TsConfigLoadOptions {
        project: project.clone(),
    });
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(
        loaded.files,
        vec![workspace_root.join("tests/tsconfig/basic/src/index.ts")]
    );

    let source = fs::read_to_string(&loaded.files[0]).unwrap();
    let diagnostics = check_source_with_options(
        &source,
        &loaded.files[0].to_string_lossy(),
        CheckerOptions {
            no_implicit_any: loaded.compiler_options.no_implicit_any,
        },
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.to_string() == "TS7006")
    );
}

#[test]
fn show_config_omits_base_url_and_keeps_paths() {
    let root = temp_dir("show-config-paths");
    write_file(
        &root,
        "tsconfig.json",
        r#"
        {
          "compilerOptions": {
            "paths": {
              "@app/*": ["src/*"]
            }
          },
          "include": ["src/**/*.ts"]
        }
        "#,
    );
    write_file(&root, "src/index.ts", "export const value = 1;");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showConfig"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("\"paths\""));
    assert!(!stdout.contains("\"baseUrl\""));

    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["compilerOptions"]["paths"]["@app/*"],
        Value::Array(vec![Value::String("src/*".to_string())])
    );
}

#[test]
fn show_config_uses_ts7_defaults() {
    let root = temp_dir("show-config-defaults");
    write_file(&root, "tsconfig.json", r#"{ "compilerOptions": {} }"#);

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showConfig"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("\"strict\": true"));
    assert!(stdout.contains("\"noImplicitAny\": true"));
    assert!(stdout.contains("\"target\": \"es2024\""));
    assert!(stdout.contains("\"module\": \"preserve\""));
    assert!(stdout.contains("\"moduleResolution\": \"bundler\""));
}

#[test]
fn project_mode_empty_config_triggers_ts7006() {
    let root = temp_dir("project-ts7006");
    write_file(
        &root,
        "tsconfig.json",
        r#"
        {
          "compilerOptions": {},
          "include": ["src/**/*.ts"]
        }
        "#,
    );
    write_file(
        &root,
        "src/index.ts",
        "function f(value): string { return \"ok\"; }",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS7006"));
}

#[test]
fn project_mode_package_extends_reports_ts7006_and_show_config_defaults() {
    let root = temp_dir("project-package-extends");
    write_file(
        &root,
        "node_modules/@tsconfig/strictest/tsconfig.json",
        r#"
        {
          "compilerOptions": {
            "strict": true,
            "target": "es2024",
            "module": "preserve",
            "moduleResolution": "bundler"
          }
        }
        "#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"
        {
          "extends": "@tsconfig/strictest",
          "include": ["src/**/*.ts"]
        }
        "#,
    );
    write_file(
        &root,
        "src/index.ts",
        "function f(value): string { return \"ok\"; }",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS7006"));

    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showConfig"]);
    assert!(stderr.is_empty());

    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["compilerOptions"]["strict"], Value::Bool(true));
    assert_eq!(
        parsed["compilerOptions"]["noImplicitAny"],
        Value::Bool(true)
    );
}
