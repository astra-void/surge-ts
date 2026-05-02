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
    assert!(stdout.contains("TS2304"));
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
    assert!(stdout.contains("Diagnostics: 3"));
    assert!(stdout.contains("TS2307  3"));
}

#[test]
fn cli_compat_report_project_counts_by_file() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/index.ts  3"));
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
    assert_eq!(parsed["diagnosticsTotal"], Value::from(3));
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
    assert!(stdout.contains("Diagnostics: 3"));
    assert!(stdout.contains("TS2307  3"));
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
    assert!(stdout.contains("TS2307  3"));
}

#[test]
fn compat_project_report_counts_by_file() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let (stdout, stderr) = run_cli(&["--project", project.as_str(), "--compatReport"]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/index.ts  3"));
}
