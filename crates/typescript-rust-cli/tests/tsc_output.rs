use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

fn temp_dir(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
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

const TSCONFIG: &str =
    r#"{ "compilerOptions": { "noEmit": true, "strict": true }, "include": ["src/**/*.ts"] }"#;

/// Run the CLI with `cwd` set to the project root so file labels are emitted
/// relative to the project (`src/index.ts`), matching how `tsc` is run.
fn run_in(root: &PathBuf, args: &[&str], force_color: bool) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"));
    command.current_dir(root).args(args);
    if force_color {
        command.env("FORCE_COLOR", "1");
    } else {
        command.env_remove("FORCE_COLOR");
        command.env_remove("NO_COLOR");
    }
    let output = command.output().unwrap();
    assert!(output.status.success() || !output.stdout.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn const_project() -> PathBuf {
    let root = temp_dir("tsc-const");
    write_file(&root, "tsconfig.json", TSCONFIG);
    write_file(&root, "src/index.ts", "const a = 1;\nexport {};\na = 3;\n");
    root
}

#[test]
fn default_non_pretty_matches_tsc_one_line() {
    let root = const_project();
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "false"],
        false,
    );
    assert_eq!(
        stdout,
        "src/index.ts(3,1): error TS2588: Cannot assign to 'a' because it is a constant.\n"
    );
}

#[test]
fn default_style_is_tsc_not_custom() {
    let root = const_project();
    // No style flag at all: the default must be tsc-compatible, not the custom
    // `error[TS....]` / ` --> ` Rust-style output.
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "false"],
        false,
    );
    assert!(stdout.contains("(3,1): error TS2588:"));
    assert!(!stdout.contains("error[TS2588]"));
    assert!(!stdout.contains(" --> "));
}

#[test]
fn pretty_true_no_color_matches_tsc_frame() {
    let root = const_project();
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "true"],
        false,
    );
    let expected = "src/index.ts:3:1 - error TS2588: Cannot assign to 'a' because it is a constant.\n\n3 a = 3;\n  ~\n\n\nFound 1 error in src/index.ts:3\n\n";
    assert_eq!(stdout, expected);
}

#[test]
fn pretty_true_color_matches_tsc_ansi() {
    let root = const_project();
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "true"],
        true,
    );
    let expected = "\x1b[96msrc/index.ts\x1b[0m:\x1b[93m3\x1b[0m:\x1b[93m1\x1b[0m - \x1b[91merror\x1b[0m\x1b[90m TS2588: \x1b[0mCannot assign to 'a' because it is a constant.\n\n\x1b[7m3\x1b[0m a = 3;\n\x1b[7m \x1b[0m \x1b[91m~\x1b[0m\n\n\nFound 1 error in src/index.ts\x1b[90m:3\x1b[0m\n\n";
    assert_eq!(stdout, expected);
}

#[test]
fn no_color_env_disables_ansi_even_when_pretty() {
    let root = const_project();
    let mut command = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"));
    command
        .current_dir(&root)
        .args(["--project", "tsconfig.json", "--pretty", "true"])
        .env("FORCE_COLOR", "1")
        .env("NO_COLOR", "1");
    let stdout = String::from_utf8(command.output().unwrap().stdout).unwrap();
    assert!(!stdout.contains('\x1b'));
    assert!(stdout.contains("src/index.ts:3:1 - error TS2588:"));
}

#[test]
fn pretty_false_equals_default_when_piped() {
    let root = const_project();
    let explicit = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "false"],
        false,
    );
    // Piped stdout (not a TTY) defaults to non-pretty, matching `--pretty false`.
    let default = run_in(&root, &["--project", "tsconfig.json"], false);
    assert_eq!(explicit, default);
}

#[test]
fn multiple_diagnostics_in_one_file() {
    let root = temp_dir("tsc-multi-one-file");
    write_file(&root, "tsconfig.json", TSCONFIG);
    write_file(
        &root,
        "src/index.ts",
        "export {};\nlet a: number = \"x\";\nlet b: number = \"y\";\n",
    );
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "false"],
        false,
    );
    let lines: Vec<&str> = stdout.lines().filter(|line| !line.is_empty()).collect();
    assert_eq!(lines.len(), 2, "expected two diagnostics, got: {stdout:?}");
    assert!(lines[0].starts_with("src/index.ts(2,"));
    assert!(lines[0].contains("error TS2322:"));
    assert!(lines[1].starts_with("src/index.ts(3,"));

    // Pretty same-file footer.
    let pretty = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "true"],
        false,
    );
    assert!(pretty.contains("Found 2 errors in the same file, starting at: src/index.ts:2"));
}

#[test]
fn multiple_files_footer_and_ordering() {
    let root = temp_dir("tsc-multi-file");
    write_file(&root, "tsconfig.json", TSCONFIG);
    write_file(&root, "src/a.ts", "export {};\nlet a: number = \"x\";\n");
    write_file(&root, "src/b.ts", "export {};\nlet b: number = \"y\";\n");

    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "false"],
        false,
    );
    let a_index = stdout.find("src/a.ts").expect("a.ts present");
    let b_index = stdout.find("src/b.ts").expect("b.ts present");
    assert!(a_index < b_index, "a.ts should be emitted before b.ts");
    assert!(stdout.contains("error TS2322:"));

    let pretty = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "true"],
        false,
    );
    assert!(pretty.contains("Found 2 errors in 2 files."));
    assert!(pretty.contains("Errors  Files"));
    assert!(pretty.contains("1  src/a.ts:2"));
    assert!(pretty.contains("1  src/b.ts:2"));
}

#[test]
fn custom_style_is_preserved_behind_flag() {
    let root = const_project();
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--diagnostic-style", "custom"],
        false,
    );
    assert!(stdout.contains("error[TS2588]"));
    assert!(stdout.contains(" --> "));
}

#[test]
fn json_style_emits_machine_readable() {
    let root = const_project();
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--diagnostic-style", "json"],
        false,
    );
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        parsed["diagnostics"][0]["code"],
        Value::String("TS2588".into())
    );
}

#[test]
fn camel_case_style_alias_also_works() {
    let root = const_project();
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--diagnosticStyle", "custom"],
        false,
    );
    assert!(stdout.contains("error[TS2588]"));
}

#[test]
fn success_prints_nothing() {
    let root = temp_dir("tsc-success");
    write_file(&root, "tsconfig.json", TSCONFIG);
    write_file(
        &root,
        "src/index.ts",
        "export {};\nlet ok: string = \"ok\";\n",
    );
    let stdout = run_in(
        &root,
        &["--project", "tsconfig.json", "--pretty", "true"],
        false,
    );
    assert!(stdout.is_empty(), "expected empty output, got: {stdout:?}");
}
