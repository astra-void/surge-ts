use std::process::Command;

#[test]
fn test_default_profile_is_tsc() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("test_tsc_{}.ts", std::process::id()));

    let source = "interface User { name: string; age?: number; }\nfunction acceptUser(u: User) {}\nacceptUser((1 as any as number) satisfies User);\n";
    std::fs::write(&file_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .arg(file_path.clone())
        .arg("--ignoreConfig")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("does not satisfy the expected type"));
    assert!(stdout.contains("is not assignable to parameter of type"));
}

#[test]
fn test_native_profile_suppresses_cascade() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("test_native_{}.ts", std::process::id()));

    let source = "interface User { name: string; age?: number; }\nfunction acceptUser(u: User) {}\nacceptUser((1 as any as number) satisfies User);\n";
    std::fs::write(&file_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .arg(file_path.clone())
        .arg("--ignoreConfig")
        .arg("--diagnosticProfile")
        .arg("native")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("does not satisfy the expected type"));
    assert!(!stdout.contains("is not assignable to parameter of type"));
}

#[test]
fn test_single_file_jobs_are_rejected_cleanly() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("test_jobs_{}.ts", std::process::id()));

    let source = "const value: string = 1;";
    std::fs::write(&file_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .arg(file_path.clone())
        .arg("--ignoreConfig")
        .arg("--jobs")
        .arg("4")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--jobs is only supported with --project"));
}
