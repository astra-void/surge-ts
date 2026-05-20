use std::{fs, path::PathBuf, process::Command, time::SystemTime};

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

fn run_cli(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn compat_report_does_not_classify_root_causes() {
    let root = temp_dir("report-no-classifier");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "strict": true, "noEmit": true }, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        import { MissingThing } from "./missing";

        export const value: MissingThing = UnknownGlobal;
        "#,
    );

    let project = root.join("tsconfig.json").to_string_lossy().into_owned();
    let text = run_cli(&["--project", project.as_str(), "--compatReport"]);
    let json = run_cli(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);
    let report_source =
        fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/report.rs"))
            .unwrap();
    let implementation_source = report_source
        .find("#[cfg(test)]")
        .map(|index| &report_source[..index])
        .unwrap_or(&report_source);

    for needle in [
        "nodeModulesSourceDiagnostics",
        "nodeModulesJavaScriptSourceDiagnostics",
        "CategorizedCountEntry",
        "candidate",
        "category",
    ] {
        assert!(!text.contains(needle), "compat report text still contains classifier text: {needle}");
        assert!(!json.contains(needle), "compat report json still contains classifier text: {needle}");
        assert!(
            !implementation_source.contains(needle),
            "report.rs still contains classifier text: {needle}"
        );
    }
}
