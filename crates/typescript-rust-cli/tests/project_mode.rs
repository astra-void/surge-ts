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

#[test]
fn project_mode_cross_file_interface_valid() {
    let root = temp_dir("project-cross-file-interface-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "interface User { name: string; }");
    write_file(&root, "src/b.ts", "let user: User = { name: \"Ada\" };");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_cross_file_interface_mismatch() {
    let root = temp_dir("project-cross-file-interface-mismatch");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "interface User { name: string; }");
    write_file(&root, "src/b.ts", "let user: User = { name: 123 };");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/b.ts"));
    assert!(stdout.contains("TS2322"));
    assert!(!stdout.contains("src/a.ts\nerror[TS2322]"));
}

#[test]
fn project_mode_cross_file_type_alias_valid() {
    let root = temp_dir("project-cross-file-type-alias-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "type Name = string;");
    write_file(&root, "src/b.ts", "let value: Name = \"Ada\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_uses_program_checker_for_cross_file_type_alias() {
    let root = temp_dir("project-cross-file-type-alias-mismatch");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "type Name = string;");
    write_file(&root, "src/b.ts", "let value: Name = 123;");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/b.ts"));
    assert!(stdout.contains("TS2322"));
    assert!(!stdout.contains("src/a.ts\nerror[TS2322]"));
}

#[test]
fn project_mode_cross_file_function_valid() {
    let root = temp_dir("project-cross-file-function-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "function getName(): string { return \"Ada\"; }",
    );
    write_file(&root, "src/b.ts", "let value: string = getName();");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_uses_program_checker_for_cross_file_function() {
    let root = temp_dir("project-cross-file-function-mismatch");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "function getName(): string { return \"Ada\"; }",
    );
    write_file(&root, "src/b.ts", "let value: number = getName();");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/b.ts"));
    assert!(stdout.contains("TS2322"));
    assert!(!stdout.contains("src/a.ts\nerror[TS2322]"));
}

#[test]
fn project_mode_cross_file_function_return_mismatch() {
    let root = temp_dir("project-cross-file-function-return-mismatch");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "function getName(): string { return \"Ada\"; }",
    );
    write_file(&root, "src/b.ts", "let value: number = getName();");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/b.ts"));
    assert!(stdout.contains("TS2322"));
    assert!(!stdout.contains("src/a.ts\nerror[TS2322]"));
}

#[test]
fn project_mode_diagnostics_grouped_by_file() {
    let root = temp_dir("project-diagnostics-grouped");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "let a: number = \"x\";");
    write_file(&root, "src/b.ts", "let b: number = \"y\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    let a_index = stdout.find("src/a.ts").expect("expected a.ts block");
    let b_index = stdout.find("src/b.ts").expect("expected b.ts block");
    assert!(a_index < b_index);
}

#[test]
fn project_mode_top_level_variable_not_shared_policy() {
    let root = temp_dir("project-top-level-variable-policy");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "let name = \"Ada\";");
    write_file(&root, "src/b.ts", "let value: string = name;");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/b.ts"));
    assert!(stdout.contains("TS2304"));
}

#[test]
fn project_mode_parser_diagnostic_grouped_by_file() {
    let root = temp_dir("project-parser-diagnostic-grouped");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "let value: string | = \"bad\";");
    write_file(&root, "src/b.ts", "let ok: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(!stdout.contains("src/b.ts\nerror[typescript-rust::parser-error]"));
}

#[test]
fn project_mode_single_file_position_arg_still_works() {
    let root = temp_dir("project-single-file-position");
    let file = root.join("index.ts");
    fs::write(&file, "let value: string = 123;").unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[file.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2322"));
}

#[test]
fn project_mode_exported_interface_not_global_yet() {
    let root = temp_dir("project-exported-interface-not-global");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "export interface User { name: string; }");
    write_file(&root, "src/b.ts", "let user: User = { name: \"Ada\" };");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/b.ts"));
    assert!(stdout.contains("TS2304"));
}

#[test]
fn project_mode_import_named_unresolved_until_resolution_phase() {
    let root = temp_dir("project-import-named-unresolved");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "import { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("TS2304"));
}

#[test]
fn project_mode_import_side_effect_valid() {
    let root = temp_dir("project-import-side-effect-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "import \"./setup\";\nlet value: string = \"ok\";",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_empty_export_marks_module_current_policy() {
    let root = temp_dir("project-empty-export-module");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "export {};\nlet value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_single_file_positional_export_valid() {
    let root = temp_dir("project-single-file-export-valid");
    let file = root.join("index.ts");
    fs::write(
        &file,
        "export interface User { name: string; }\nlet user: User = { name: \"Ada\" };",
    )
    .unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[file.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}
