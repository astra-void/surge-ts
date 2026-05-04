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
    let stdout = String::from_utf8(output.stdout).unwrap();
    let normalize_paths = !args
        .windows(2)
        .any(|window| window[0] == "--format" && window[1] == "json");
    (
        if normalize_paths {
            stdout.replace('\\', "/")
        } else {
            stdout
        },
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn run_cli_raw(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn compat_project_root(name: &str) -> PathBuf {
    workspace_root().join("tests/compat-projects").join(name)
}

fn run_cli_json(args: &[&str]) -> Value {
    let (stdout, stderr) = run_cli(args);
    assert!(stderr.is_empty());
    serde_json::from_str(&stdout).unwrap()
}

fn json_diagnostics(parsed: &Value) -> &[Value] {
    parsed["diagnostics"]
        .as_array()
        .map(|items| items.as_slice())
        .unwrap()
}

fn json_diagnostic_codes(parsed: &Value) -> Vec<String> {
    json_diagnostics(parsed)
        .iter()
        .map(|diagnostic| diagnostic["code"].as_str().unwrap().to_string())
        .collect()
}

fn json_diagnostic_lines(parsed: &Value, code: &str) -> Vec<Option<u64>> {
    json_diagnostics(parsed)
        .iter()
        .filter(|diagnostic| diagnostic["code"].as_str() == Some(code))
        .map(|diagnostic| diagnostic["line"].as_u64())
        .collect()
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
            stub_external_modules: false,
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
        "src/user.ts",
        "export interface User { name: string; }",
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
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_import_side_effect_valid() {
    let root = temp_dir("project-import-side-effect-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/setup.ts", "export {};");
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
    assert!(stdout.contains("No errors."));
}

#[test]
fn project_mode_single_file_positional_does_not_resolve_external_files() {
    let root = temp_dir("project-single-file-no-external-resolution");
    let file = root.join("index.ts");
    fs::write(
        &file,
        "import { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
    )
    .unwrap();
    fs::write(
        root.join("user.ts"),
        "export interface User { name: string; }",
    )
    .unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[file.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2304"));
}

#[test]
fn project_mode_import_named_unresolved_grouped_by_file() {
    let root = temp_dir("project-import-named-unresolved-grouped");
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
    write_file(&root, "src/b.ts", "let value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("TS2307"));
    assert!(!stdout.contains("src/b.ts\nerror[TS2307]"));
}

#[test]
fn project_mode_import_type_named_unresolved_grouped_by_file() {
    let root = temp_dir("project-import-type-unresolved-grouped");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "import type { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
    );
    write_file(&root, "src/b.ts", "let value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("TS2307"));
    assert!(!stdout.contains("src/b.ts\nerror[TS2307]"));
}

#[test]
fn project_mode_relative_interface_import_valid() {
    let root = temp_dir("project-relative-interface-import-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/user.ts",
        "export interface User { name: string; }",
    );
    write_file(
        &root,
        "src/index.ts",
        "import { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_side_effect_import_script_file_valid() {
    let root = temp_dir("project-side-effect-script-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/setup.ts", "let initialized: boolean = true;");
    write_file(
        &root,
        "src/index.ts",
        "import \"./setup\";\nlet value: string = \"ok\";",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_named_import_from_script_file_reports_missing_export() {
    let root = temp_dir("project-named-import-script-missing-export");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/setup.ts", "let value = 1;");
    write_file(&root, "src/index.ts", "import { value } from \"./setup\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2305"));
    assert!(stdout.contains("src/index.ts"));
}

#[test]
fn project_mode_relative_type_alias_import_valid() {
    let root = temp_dir("project-relative-type-alias-import-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/user.ts", "export type UserId = string;");
    write_file(
        &root,
        "src/index.ts",
        "import type { UserId } from \"./user\";\nlet id: UserId = \"u1\";",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_default_import_cross_file_valid() {
    let root = temp_dir("project-default-import-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/user.ts",
        "export default function getName(): string { return \"Ada\"; }",
    );
    write_file(
        &root,
        "src/index.ts",
        "import getName from \"./user\";\nlet value: string = getName();",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_namespace_import_cross_file_valid() {
    let root = temp_dir("project-namespace-import-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/user.ts", "export const version: number = 1;");
    write_file(
        &root,
        "src/index.ts",
        "import * as user from \"./user\";\nlet value: number = user.version;",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_star_re_export_missing_module_no_consumer_cascade() {
    let root = temp_dir("project-star-re-export-missing-module");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "export * from \"./missing\";");
    write_file(
        &root,
        "src/app.ts",
        "import { User } from \"./index\";\nlet value = User;",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2307"));
    assert!(!stdout.contains("TS2305"));
}

#[test]
fn project_mode_regular_type_export_value_usage_unresolved() {
    let root = temp_dir("project-regular-type-export-value-usage");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/user.ts",
        "export interface User { name: string; }",
    );
    write_file(
        &root,
        "src/index.ts",
        "import { User } from \"./user\";\nlet value = User;",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2693"));
    assert!(stdout.contains("src/index.ts"));
}

#[test]
fn project_mode_regular_value_export_type_usage_unresolved() {
    let root = temp_dir("project-regular-value-export-type-usage");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/user.ts", "export const User: string = \"Ada\";");
    write_file(
        &root,
        "src/index.ts",
        "import { User } from \"./user\";\nlet value: User = \"Ada\";",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2304"));
    assert!(stdout.contains("src/index.ts"));
}

#[test]
fn project_mode_relative_function_import_valid() {
    let root = temp_dir("project-relative-function-import-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/user.ts",
        "export function getName(): string { return \"Ada\"; }",
    );
    write_file(
        &root,
        "src/index.ts",
        "import { getName } from \"./user\";\nlet name: string = getName();",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_show_spans_module_missing_export() {
    let root = temp_dir("project-show-spans-missing-export");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/user.ts",
        "export interface User { name: string; }",
    );
    write_file(&root, "src/index.ts", "import { Missing } from \"./user\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showSpans"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2305"));
    assert!(stdout.contains("start="));
    assert!(stdout.contains("end="));
}

#[test]
fn project_mode_show_spans_module_missing_relative() {
    let root = temp_dir("project-show-spans-missing-relative");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "import { User } from \"./missing\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showSpans"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2307"));
    assert!(stdout.contains("start="));
    assert!(stdout.contains("end="));
}

#[test]
fn project_mode_relative_variable_import_valid() {
    let root = temp_dir("project-relative-variable-import-valid");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/user.ts",
        "export const version: string = \"1\";",
    );
    write_file(
        &root,
        "src/index.ts",
        "import { version } from \"./user\";\nlet current: string = version;",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_relative_missing_export_grouped_by_importer_file() {
    let root = temp_dir("project-relative-missing-export-grouped");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/user.ts",
        "export interface User { name: string; }",
    );
    write_file(
        &root,
        "src/index.ts",
        "import { Missing } from \"./user\";\nlet value: Missing = \"x\";",
    );
    write_file(&root, "src/other.ts", "let value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/index.ts"));
    assert!(stdout.contains("TS2305"));
    assert!(!stdout.contains("src/other.ts\nerror[TS2305]"));
}

#[test]
fn project_mode_relative_export_declaration_error_grouped_by_exporter_file() {
    let root = temp_dir("project-relative-export-error-grouped");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/user.ts", "export type Name = Missing;");
    write_file(
        &root,
        "src/index.ts",
        "import { Name } from \"./user\";\nlet value: Name = \"Ada\";",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/user.ts"));
    assert!(stdout.contains("TS2304"));
    assert!(!stdout.contains("src/index.ts\nerror[TS2304]"));
}

#[test]
fn project_mode_show_spans_relative_import_error() {
    let root = temp_dir("project-relative-show-spans-error");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "import { User } from \"./missing\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showSpans"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2307"));
    assert!(stdout.contains("start="));
    assert!(stdout.contains("end="));
}

#[test]
fn project_mode_export_empty_valid() {
    let root = temp_dir("project-export-empty-valid");
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
fn project_mode_exported_interface_same_file_valid() {
    let root = temp_dir("project-exported-interface-same-file");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "export interface User { name: string; }\nlet user: User = { name: \"Ada\" };",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn project_mode_exported_interface_not_global() {
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
fn project_mode_module_file_does_not_see_script_global_current_policy() {
    let root = temp_dir("project-module-does-not-see-script-global");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "interface User { name: string; }");
    write_file(
        &root,
        "src/b.ts",
        "export {};\nlet user: User = { name: \"Ada\" };",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/b.ts"));
    assert!(stdout.contains("TS2304"));
}

#[test]
fn project_mode_script_files_still_share_global_interface() {
    let root = temp_dir("project-script-share-interface");
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
fn project_mode_malformed_import_parser_error_grouped_by_file() {
    let root = temp_dir("project-malformed-import");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "import { User from \"./user\";");
    write_file(&root, "src/b.ts", "let value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("typescript-rust::parser-error"));
    assert!(!stdout.contains("src/b.ts\nerror[typescript-rust::parser-error]"));
}

#[test]
fn project_mode_malformed_export_parser_error_grouped_by_file() {
    let root = temp_dir("project-malformed-export");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "export { User;");
    write_file(&root, "src/b.ts", "let value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("typescript-rust::parser-error"));
    assert!(!stdout.contains("src/b.ts\nerror[typescript-rust::parser-error]"));
}

#[test]
fn project_mode_single_file_positional_module_syntax_valid() {
    let root = temp_dir("project-single-file-module-valid");
    let file = root.join("index.ts");
    fs::write(
        &file,
        "export interface User { name: string; }\nlet user: User = { name: \"Ada\" };",
    )
    .unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[file.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("No errors."));
}

#[test]
fn cli_show_spans_single_file_includes_start_end() {
    let root = temp_dir("single-file-show-spans");
    let file = root.join("index.ts");
    fs::write(&file, "let value: number = \"a\";").unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--showSpans", file.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2322"));
    assert!(stdout.contains("start="));
    assert!(stdout.contains("end="));
}

#[test]
fn cli_show_spans_single_file_normal_output_unchanged_without_flag() {
    let root = temp_dir("single-file-show-spans-normal");
    let file = root.join("index.ts");
    fs::write(&file, "let value: number = \"a\";").unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[file.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2322"));
    assert!(!stdout.contains("start="));
    assert!(!stdout.contains("end="));
}

#[test]
fn cli_show_spans_project_mode_groups_by_file_if_supported() {
    let root = temp_dir("project-show-spans");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "let value: number = \"a\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showSpans"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/index.ts"));
    assert!(stdout.contains("TS2322"));
    assert!(stdout.contains("start="));
}

#[test]
fn cli_show_spans_show_config_still_exits_successfully() {
    let root = temp_dir("project-show-spans-config");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "let value: number = \"a\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showSpans", "--showConfig"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("\"compilerOptions\""));
    assert!(!stdout.contains("start="));
}

#[test]
fn cli_show_config_still_exits_successfully() {
    let root = temp_dir("project-show-config-success");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "let value: number = \"a\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--showConfig"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("\"compilerOptions\""));
}

#[test]
fn cli_show_spans_still_works() {
    let root = temp_dir("single-file-show-spans-still-works");
    let file = root.join("index.ts");
    fs::write(&file, "let value: number = \"a\";").unwrap();

    let file = file.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--showSpans", file.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2322"));
    assert!(stdout.contains("start="));
    assert!(stdout.contains("end="));
}

#[test]
fn cli_project_normal_output_unchanged_without_compat_report() {
    let root = temp_dir("project-normal-output-unchanged");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/index.ts", "let value: number = \"a\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2322"));
    assert!(!stdout.contains("Compatibility report"));
}

#[test]
fn project_mode_non_relative_import_grouped_by_importer_file() {
    let root = temp_dir("project-non-relative-import-grouped");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/a.ts",
        "import { User } from \"pkg\";\nlet user: User = { name: 123 };",
    );
    write_file(&root, "src/b.ts", "let value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("TS2307"));
    assert!(!stdout.contains("src/b.ts\nerror[TS2307]"));
}

#[test]
fn cli_max_diagnostics_limits_rendered_output() {
    let root = temp_dir("project-max-diagnostics");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "let a: number = \"a\";");
    write_file(&root, "src/b.ts", "let b: number = \"b\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--maxDiagnostics", "1"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("Showing first 1 of 2 diagnostics."));
    assert!(!stdout.contains("src/b.ts"));
}

#[test]
fn cli_compat_report_project_counts_by_code() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Compatibility report"));
    assert!(stdout.contains("Files loaded: 1"));
    assert!(stdout.contains("Diagnostics: 8"));
    assert!(stdout.contains("TS2307  7"));
    assert!(stdout.contains("TS2882  1"));
}

#[test]
fn cli_compat_report_project_counts_by_file() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/index.ts  8"));
}

#[test]
fn cli_compat_report_includes_files_loaded() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Files loaded: 1"));
}

#[test]
fn cli_compat_report_includes_parser_error_count() {
    let root = temp_dir("project-parser-error-count");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "import { User from \"./user\";");
    write_file(&root, "src/b.ts", "let value: string = \"ok\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Parser errors: 1"));
    assert!(stdout.contains("typescript-rust::parser-error"));
}

#[test]
fn cli_compat_report_with_max_diagnostics_counts_all() {
    let root = temp_dir("project-compat-max-diagnostics");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "let a: number = \"a\";");
    write_file(&root, "src/b.ts", "let b: number = \"b\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--maxDiagnostics",
        "1",
    ]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Diagnostics: 2"));
    assert!(stdout.contains("TS2322  2"));
    assert!(stdout.contains("Showing first 1 of 2 diagnostics."));
}

#[test]
fn cli_compat_report_format_json_still_report_shape() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
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
    let expected_root = compat_project_root("package-imports")
        .to_string_lossy()
        .to_string();
    assert_eq!(parsed["rootDir"].as_str().unwrap(), expected_root);
    assert_eq!(parsed["filesLoaded"], Value::from(1));
    assert_eq!(parsed["diagnosticsTotal"], Value::from(8));
    assert!(parsed["byCode"].is_array());
    assert!(parsed["byFile"].is_array());
    assert!(parsed["parserErrors"].is_array());
    assert_eq!(
        parsed["byCode"][0]["code"],
        Value::String("TS2307".to_string())
    );
}

#[test]
fn cli_max_diagnostics_limits_json_diagnostics_but_not_report_counts() {
    let root = temp_dir("project-json-max-diagnostics");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "let a: number = \"a\";");
    write_file(&root, "src/b.ts", "let b: number = \"b\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let (diagnostics_stdout, diagnostics_stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--maxDiagnostics",
        "1",
    ]);
    assert!(diagnostics_stderr.is_empty());
    let diagnostics_json: Value = serde_json::from_str(&diagnostics_stdout).unwrap();
    assert_eq!(diagnostics_json["diagnostics"].as_array().unwrap().len(), 1);

    let (report_stdout, report_stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
        "--maxDiagnostics",
        "1",
    ]);
    assert!(report_stderr.is_empty());
    let report_json: Value = serde_json::from_str(&report_stdout).unwrap();
    assert_eq!(report_json["diagnosticsTotal"], Value::from(2));
    assert_eq!(report_json["byCode"][0]["count"], Value::from(2));
}

#[test]
fn cli_max_diagnostics_zero_or_invalid_rejected_or_pinned() {
    let root = temp_dir("project-max-diagnostics-zero");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "let a: number = \"a\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let output = run_cli_raw(&["--project", project.as_str(), "--maxDiagnostics", "0"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--maxDiagnostics must be greater than 0"));
}

#[test]
fn compat_project_package_imports_report_stable() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Compatibility report"));
    assert!(stdout.contains("Diagnostics: 8"));
    assert!(stdout.contains("TS2307  7"));
    assert!(stdout.contains("TS2882  1"));
}

#[test]
fn package_imports_line5_ts2882_matches_typescript() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert_eq!(json_diagnostic_lines(&parsed, "TS2882"), vec![Some(5)]);
}

#[test]
fn package_imports_default_no_extra_ts2307_for_ts2882_case() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let mut ts2307_lines = json_diagnostic_lines(&parsed, "TS2307");
    ts2307_lines.sort();

    assert_eq!(
        ts2307_lines,
        vec![
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(7),
            Some(8),
            Some(9),
        ]
    );
}

#[test]
fn package_imports_other_package_imports_remain_ts2307_cli() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert_eq!(json_diagnostic_lines(&parsed, "TS2307").len(), 7);
}

#[test]
fn package_imports_stub_external_modules_ts2882_policy_pinned_cli() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--stubExternalModules",
        "--format",
        "json",
    ]);

    assert!(json_diagnostic_lines(&parsed, "TS2307").is_empty());
    assert!(json_diagnostic_lines(&parsed, "TS2882").is_empty());
}

#[test]
fn compat_project_module_forms_no_panic() {
    let project = compat_project_root("module-forms").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Compatibility report"));
    assert!(stdout.contains("typescript-rust::unsupported-module-syntax"));
}

#[test]
fn compat_project_relative_deep_valid() {
    let project = compat_project_root("relative-deep").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn compat_project_private_types_valid() {
    let project = compat_project_root("private-types").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str()]);

    assert!(stderr.is_empty());
    assert!(stdout.trim().is_empty());
}

#[test]
fn compat_project_report_counts_by_code() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("TS2307  7"));
    assert!(stdout.contains("TS2882  1"));
}

#[test]
fn compat_project_report_counts_by_file() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/index.ts  8"));
}

#[test]
fn cli_stub_external_modules_project_suppresses_package_ts2307() {
    let root = temp_dir("cli_stub_project");
    write_file(&root, "tsconfig.json", r#"{ "include": ["*.ts"] }"#);
    write_file(&root, "index.ts", r#"import { useState } from "react";"#);
    let (stdout, stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
    ]);
    assert!(stdout.contains("TS2307"));
    assert!(stderr.is_empty());

    let (stdout, stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
        "--stubExternalModules",
    ]);
    assert!(!stdout.contains("TS2307"));
    assert!(stderr.is_empty());
}

#[test]
fn cli_stub_external_modules_project_keeps_relative_ts2307() {
    let root = temp_dir("cli_stub_project_rel");
    write_file(&root, "tsconfig.json", r#"{ "include": ["*.ts"] }"#);
    write_file(&root, "index.ts", r#"import { X } from "./missing";"#);

    let (stdout, stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
        "--stubExternalModules",
    ]);
    assert!(stdout.contains("TS2307"));
    assert!(stderr.is_empty());
}

#[test]
fn cli_stub_external_modules_single_file_ignore_config_suppresses_package_ts2307() {
    let root = temp_dir("cli_stub_single");
    let file = root.join("index.ts");
    fs::write(&file, r#"import { useState } from "react";"#).unwrap();

    let (stdout, _stderr) = run_cli(&["--ignoreConfig", file.to_string_lossy().as_ref()]);
    assert!(stdout.contains("TS2307"));

    let (stdout, _stderr) = run_cli(&[
        "--ignoreConfig",
        file.to_string_lossy().as_ref(),
        "--stubExternalModules",
    ]);
    assert!(!stdout.contains("TS2307"));
}

#[test]
fn cli_stub_external_modules_does_not_affect_ts5112() {
    let root = temp_dir("cli_stub_ts5112");
    write_file(&root, "tsconfig.json", r#"{ "include": ["*.ts"] }"#);
    let file = root.join("index.ts");
    fs::write(&file, "let x = 1;").unwrap();

    // Changing the CWD so the CLI detects tsconfig.json automatically
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"));
    cmd.current_dir(&root);
    cmd.arg("index.ts");
    cmd.arg("--stubExternalModules");
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("TS5112"));
}

#[test]
fn cli_stub_external_modules_compat_report() {
    let root = temp_dir("cli_stub_compat_report");
    write_file(&root, "tsconfig.json", r#"{ "include": ["*.ts"] }"#);
    write_file(
        &root,
        "index.ts",
        r#"import { useState } from "react"; import { create } from "zustand";"#,
    );

    let (stdout, _stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
        "--compatReport",
    ]);
    assert!(stdout.contains("External module stubs: 2"));
    assert!(stdout.contains("react  1"));
    assert!(stdout.contains("zustand  1"));
    assert!(stdout.contains("TS2307"));

    let (stdout, _stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
        "--compatReport",
        "--stubExternalModules",
    ]);
    assert!(stdout.contains("External module stubs: 2"));
    assert!(!stdout.contains("TS2307"));
}

#[test]
fn cli_default_external_import_reports_ts2307_no_cascade() {
    let root = temp_dir("cli_default_ext");
    write_file(&root, "tsconfig.json", r#"{ "include": ["*.ts"] }"#);
    write_file(
        &root,
        "index.ts",
        r#"import * as Zustand from "zustand"; let x = Zustand.create;"#,
    );

    let (stdout, _stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
    ]);
    assert!(stdout.contains("TS2307"));
    assert!(!stdout.contains("TS2339"));
}

#[test]
fn cli_external_namespace_property_access_no_cascade() {
    let root = temp_dir("cli_ext_ns");
    let file = root.join("index.ts");
    fs::write(
        &file,
        r#"import * as Zustand from "zustand"; let store = Zustand.createStore;"#,
    )
    .unwrap();

    let (stdout, _stderr) = run_cli(&["--ignoreConfig", file.to_string_lossy().as_ref()]);
    assert!(stdout.contains("TS2307"));
    assert!(!stdout.contains("TS2339"));
}

#[test]
fn compat_report_external_module_stubs_json() {
    let root = temp_dir("cli_stub_compat_report_json");
    write_file(&root, "tsconfig.json", r#"{ "include": ["*.ts"] }"#);
    write_file(
        &root,
        "index.ts",
        r#"import { useState } from "react"; import { create } from "zustand";"#,
    );

    let (stdout, _stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
        "--compatReport",
        "--format",
        "json",
    ]);

    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let stubs = report.get("externalModuleStubs").unwrap();
    assert_eq!(stubs.get("total").unwrap().as_u64().unwrap(), 2);
    let by_specifier = stubs.get("bySpecifier").unwrap().as_array().unwrap();
    assert_eq!(by_specifier.len(), 2);

    // Sort or check for both since HashMap iteration order is non-deterministic
    let has_react = by_specifier.iter().any(|v| {
        v.get("specifier").unwrap().as_str().unwrap() == "react"
            && v.get("count").unwrap().as_u64().unwrap() == 1
    });
    let has_zustand = by_specifier.iter().any(|v| {
        v.get("specifier").unwrap().as_str().unwrap() == "zustand"
            && v.get("count").unwrap().as_u64().unwrap() == 1
    });

    assert!(has_react);
    assert!(has_zustand);
}

#[test]
fn cli_project_loads_d_ts_files() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--compatReport",
        "--format",
        "json",
    ]);
    assert_eq!(parsed["declarationFilesLoaded"], Value::from(2));

    let ambient_modules = parsed["ambientExternalModules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        ambient_modules,
        vec!["pkg".to_string(), "pkg/subpath".to_string()]
    );
}

#[test]
fn cli_project_declaration_global_type_valid() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2304");
    assert!(!lines.contains(&Some(5)));
    assert!(!lines.contains(&Some(8)));
}

#[test]
fn cli_project_declaration_global_function_valid() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2304");
    assert!(!lines.contains(&Some(17)));
}

#[test]
fn cli_project_ambient_module_import_valid() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--compatReport",
        "--format",
        "json",
    ]);

    let ambient_modules = parsed["ambientExternalModules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        ambient_modules,
        vec!["pkg".to_string(), "pkg/subpath".to_string()]
    );
}

#[test]
fn cli_project_ambient_module_missing_export() {
    let root = temp_dir("cli_project_ambient_module_missing_export");
    write_file(
        &root,
        "tsconfig.json",
        r#"{
          "include": ["src/**/*.ts", "types/**/*.d.ts"]
        }"#,
    );
    write_file(&root, "src/index.ts", "import { missing } from \"pkg\";");
    write_file(
        &root,
        "types/pkg.d.ts",
        "declare module \"pkg\" { export const foo: number; }",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    assert_eq!(json_diagnostic_lines(&parsed, "TS2305"), vec![Some(1)]);
}

#[test]
fn cli_project_ambient_module_unknown_package_fallback_default() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_lines(&parsed, "TS2307"), vec![Some(3)]);
}

#[test]
fn cli_project_ambient_module_unknown_package_fallback_stub_external_modules() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--stubExternalModules",
        "--format",
        "json",
    ]);
    assert!(json_diagnostic_lines(&parsed, "TS2307").is_empty());
    assert!(json_diagnostic_codes(&parsed).contains(&"TS2322".to_string()));
}

#[test]
fn cli_project_declaration_compat_report() {
    let (stdout, _) = run_cli(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--compatReport",
    ]);
    assert!(stdout.contains("Declaration files loaded"));
}

#[test]
fn cli_project_declaration_format_json() {
    let (stdout, _) = run_cli(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--compatReport",
        "--format",
        "json",
    ]);
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["declarationFilesLoaded"], Value::from(2));

    let ambient_modules = parsed["ambientExternalModules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        ambient_modules,
        vec!["pkg".to_string(), "pkg/subpath".to_string()]
    );
}
#[test]
fn cli_declarations_basic_loads_globals_d_ts() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(!codes.contains(&"TS2304".to_string()));
    assert!(codes.contains(&"TS2322".to_string()));
}

#[test]
fn cli_declarations_basic_loads_pkg_d_ts() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    let pkg_diagnostics = json_diagnostics(&parsed)
        .iter()
        .filter(|diagnostic| diagnostic["code"].as_str() == Some("TS2307"))
        .count();
    assert_eq!(pkg_diagnostics, 1);
    assert!(codes.contains(&"TS2307".to_string()));
}

#[test]
fn cli_declarations_basic_no_ts2307_for_declared_pkg() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    assert_eq!(lines, vec![Some(3)]);
}

#[test]
fn cli_declarations_basic_no_ts2307_for_declared_subpath() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    assert!(!lines.contains(&Some(2)));
}

#[test]
fn cli_declarations_basic_missing_pkg_fallback_ts2307() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    assert_eq!(lines, vec![Some(3)]);
}

#[test]
fn cli_declarations_basic_stub_external_modules_suppresses_only_missing_pkg() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--stubExternalModules",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(!codes.contains(&"TS2307".to_string()));
    assert!(codes.contains(&"TS2322".to_string()));
}

#[test]
fn cli_declarations_basic_format_json_stable() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let diagnostics = json_diagnostics(&parsed);
    assert!(!diagnostics.is_empty());
    for diagnostic in diagnostics {
        assert!(diagnostic.get("code").is_some());
        assert!(diagnostic.get("fileName").is_some());
        assert!(diagnostic.get("message").is_some());
    }
}

#[test]
fn cli_declarations_hardening_loads_ambient_modules() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-hardening/tsconfig.json",
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["declarationFilesLoaded"], Value::from(1));

    let ambient_modules = parsed["ambientExternalModules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        ambient_modules,
        vec![
            "barrel-pkg".to_string(),
            "barrel-star-pkg".to_string(),
            "barrel-type-pkg".to_string(),
            "merge-pkg".to_string(),
            "pkg-default".to_string(),
            "pkg-default-function".to_string(),
            "pkg-ns".to_string(),
            "source-pkg".to_string(),
        ]
    );
}

#[test]
fn cli_declarations_hardening_no_diagnostics() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/declarations-hardening/tsconfig.json",
        "--format",
        "json",
    ]);

    assert!(json_diagnostic_codes(&parsed).is_empty());
}
