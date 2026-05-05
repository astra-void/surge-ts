use std::path::PathBuf;
use std::process::Command;

#[test]
fn test_default_profile_is_tsc() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_dir = manifest_dir.parent().unwrap().parent().unwrap();

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("test_{}.ts", std::process::id()));

    // This code emits TWO TS2322 in tsc profile (one from satisfies inner, one from outer assignment)
    // but only ONE in native profile (because native suppresses the outer one by returning Unknown).
    let source = "interface User { name: string; age?: number; }; const a: User = { name: 123 } satisfies User;";
    std::fs::write(&file_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .arg(file_path.clone())
        .arg("--ignoreConfig")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    // In Tsc profile, we should see both the inner and outer error.
    assert!(stdout.contains("Type '123' is not assignable to type 'string'"));
    assert!(stdout.contains("is not assignable to type '{ age?: number; name: string; }'"));
}

#[test]
fn test_native_profile_suppresses_cascade() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root_dir = manifest_dir.parent().unwrap().parent().unwrap();

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(format!("test_{}.ts", std::process::id()));

    // In Native profile, the satisfies failure returns Unknown, suppressing the outer assignment check.
    let source = "interface User { name: string; age?: number; }; const a: User = { name: 123 } satisfies User;";
    std::fs::write(&file_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_typescript-rust-cli"))
        .arg(file_path.clone())
        .arg("--ignoreConfig")
        .arg("--diagnosticProfile")
        .arg("native")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    // We should see the inner error (TS2322 or TS1360 from inner contextual check depending on what it emits).
    // Actually, evaluate_object_literal_with_expected_type emits TS2322 for the inner mismatched property.
    assert!(stdout.contains("Type '123' is not assignable to type 'string'"));

    // But we should NOT see the outer assignment error because it was suppressed.
    assert!(!stdout.contains("is not assignable to type '{ age?: number; name: string; }'"));
}
