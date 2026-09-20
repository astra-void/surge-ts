use std::{fs, path::PathBuf, process::Command, process::Output};

const ENV_VAR: &str = "SURGE_MAX_FOOTPRINT_MB";

fn fixture_file(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    let path = root.join("index.ts");
    fs::write(&path, "let value: string = 123;\n").unwrap();
    path
}

fn run_with_limit(limit: &str, file: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surge"))
        .env(ENV_VAR, limit)
        .arg(file)
        .output()
        .unwrap()
}

#[test]
fn exceeding_the_limit_exits_137_with_a_message() {
    let file = fixture_file("memory-limit-exceeded");
    // Any live process already exceeds 1 MiB, so the synchronous first sample
    // fires before checking starts.
    let output = run_with_limit("1", &file);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(137), "stderr: {stderr}");
    assert!(
        stderr.contains("surge: memory limit exceeded") && stderr.contains(ENV_VAR),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("last stage: before check"),
        "stderr: {stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "no diagnostics once the guard fires"
    );
}

#[test]
fn a_generous_limit_leaves_the_run_untouched() {
    let file = fixture_file("memory-limit-generous");
    let guarded = run_with_limit("1048576", &file);
    let unguarded = Command::new(env!("CARGO_BIN_EXE_surge"))
        .env_remove(ENV_VAR)
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(guarded.status.code(), unguarded.status.code());
    assert_eq!(guarded.status.code(), Some(2), "the fixture has one error");
    assert_eq!(guarded.stdout, unguarded.stdout);
    assert!(
        !String::from_utf8_lossy(&guarded.stderr).contains("memory limit"),
        "the guard must stay silent when it does not fire"
    );
}

#[test]
fn a_malformed_limit_is_a_usage_error() {
    let file = fixture_file("memory-limit-malformed");
    for bad in ["0", "8g", "", "abc"] {
        let output = run_with_limit(bad, &file);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{bad:?}: stderr: {stderr}");
        assert!(stderr.contains(ENV_VAR), "{bad:?}: stderr: {stderr}");
        assert!(output.stdout.is_empty(), "{bad:?}: nothing must be checked");
    }
}
