//! A deferred intersection merge that reaches itself while it is being merged
//! must terminate. Runs tests/compat-projects/lazy-intersection-reentry-basic,
//! reduced from `@xata.io/client`: before the fix the checker blocked forever
//! on the merge's own `OnceLock`, without using CPU, so the child is polled
//! against a deadline and killed rather than left to hang the test run. The
//! reduction needs the bundled lib (`Readonly`, `Omit`, `Awaited`,
//! `ReturnType`), which is why it runs the CLI in project mode.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn self_reaching_intersection_merge_terminates() {
    let tsconfig = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/compat-projects/lazy-intersection-reentry-basic/tsconfig.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_surge"))
        .args(["--project", tsconfig.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("checking a self-reaching intersection merge did not terminate");
        }
        thread::sleep(Duration::from_millis(50));
    };

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(status.code(), Some(2), "{stdout}");
    let reported: Vec<&str> = stdout.lines().filter(|line| line.contains("error TS")).collect();
    assert_eq!(reported.len(), 1, "{stdout}");
    assert!(reported[0].contains("src/index.ts(4,14): error TS2322"), "{stdout}");
}
