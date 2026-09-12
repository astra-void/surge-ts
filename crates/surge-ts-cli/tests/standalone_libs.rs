//! The released binary must check a project with no TypeScript installation
//! anywhere: no `typescript` package, no `node_modules`, no `package.json`, and
//! no surge source tree. These tests run the real CLI in a scratch directory and
//! assert the bundled standard library is what answers.

use std::{fs, path::Path, path::PathBuf, process::Command, time::SystemTime};

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

/// Fails if any ancestor of `dir` could supply TypeScript declarations, which
/// would make a passing result meaningless.
fn assert_no_typescript_installation(dir: &Path) {
    let mut current = Some(dir);
    while let Some(path) = current {
        for name in ["node_modules", "package.json", "typescript"] {
            assert!(
                !path.join(name).exists(),
                "{} exists at {}; the standalone fixture is not isolated",
                name,
                path.display()
            );
        }
        current = path.parent();
    }
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(prefix: &str, tsconfig: &str, sources: &[(&str, &str)]) -> Self {
        let root = temp_dir(prefix);
        fs::write(root.join("tsconfig.json"), tsconfig).unwrap();
        for (name, contents) in sources {
            fs::write(root.join(name), contents).unwrap();
        }
        assert_no_typescript_installation(&root);
        Self { root }
    }

    fn check(&self, extra_args: &[&str]) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_surge"));
        command
            .current_dir(&self.root)
            .arg("--project")
            .arg(self.root.join("tsconfig.json"))
            .args(extra_args);
        let output = command.output().unwrap();
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const ES2022_TSCONFIG: &str =
    r#"{ "compilerOptions": { "target": "ES2022", "strict": true, "noEmit": true } }"#;

#[test]
fn checks_builtin_declarations_with_no_typescript_installed() {
    let fixture = Fixture::new(
        "surge-standalone-ok",
        ES2022_TSCONFIG,
        &[(
            "index.ts",
            r#"const xs: string[] = ["hello"];

const p: Promise<number> = Promise.resolve(123);

const m = new Map<string, number>();
m.set("foo", 1);

async function test(): Promise<number> {
  return await p;
}

const keep = [xs, m, test];
"#,
        )],
    );

    let (code, stdout, stderr) = fixture.check(&[]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.trim().is_empty(), "unexpected diagnostics: {stdout}");
    for complaint in ["could not find", "lib*.d.ts", "node_modules", "falling back"] {
        assert!(
            !stderr.contains(complaint),
            "stderr mentions {complaint:?}: {stderr}"
        );
    }
}

/// The embedded declarations must actually participate in checking, not merely
/// silence missing-global errors.
#[test]
fn embedded_declarations_are_type_checked() {
    let fixture = Fixture::new(
        "surge-standalone-bad",
        ES2022_TSCONFIG,
        &[("index.ts", "const xs: string[] = [];\n\nxs.push(123);\n")],
    );

    let (code, stdout, stderr) = fixture.check(&[]);
    assert_eq!(code, 2, "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        stdout.contains("error TS2345")
            && stdout.contains("Argument of type 'number' is not assignable"),
        "expected tsc's TS2345 for xs.push(123), got: {stdout}"
    );
}

#[test]
fn utility_types_and_collections_resolve_from_the_snapshot() {
    let fixture = Fixture::new(
        "surge-standalone-utility",
        ES2022_TSCONFIG,
        &[(
            "index.ts",
            r#"const set = new Set<number>();
const partial: Partial<{ a: string }> = {};
type R = Record<string, number>;
const record: R = {};

async function foo() {
  const value = await Promise.resolve(1);
  return value;
}

const keep = [set, partial, record, foo];
"#,
        )],
    );

    let (code, stdout, stderr) = fixture.check(&[]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
}

#[test]
fn dom_lib_is_not_loaded_unless_selected() {
    const SOURCE: &str = r##"const el: HTMLElement | null = document.querySelector("#app");
fetch("/api");
const controller = new AbortController();
const keep = [el, controller];
"##;

    let without_dom = Fixture::new(
        "surge-standalone-no-dom",
        r#"{ "compilerOptions": { "target": "ES2022", "lib": ["ES2022"], "strict": true, "noEmit": true } }"#,
        &[("index.ts", SOURCE)],
    );
    let (code, stdout, _) = without_dom.check(&[]);
    assert_eq!(code, 2, "expected DOM globals to be absent: {stdout}");
    for name in ["HTMLElement", "document", "fetch", "AbortController"] {
        assert!(
            stdout.contains(&format!("Cannot find name '{name}'")),
            "expected a missing-global error for {name}, got: {stdout}"
        );
    }

    let with_dom = Fixture::new(
        "surge-standalone-dom",
        r#"{ "compilerOptions": { "target": "ES2022", "lib": ["ES2022", "DOM"], "strict": true, "noEmit": true } }"#,
        &[("index.ts", SOURCE)],
    );
    let (code, stdout, stderr) = with_dom.check(&[]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
}

/// `lib` names are matched the way TypeScript matches them: case-insensitively,
/// with or without the `lib.`/`.d.ts` affixes.
#[test]
fn lib_names_are_normalized_like_typescript() {
    for lib in [r#""ES2022", "DOM.Iterable", "DOM""#, r#""es2022", "dom""#] {
        let fixture = Fixture::new(
            "surge-standalone-libname",
            &format!(
                r#"{{ "compilerOptions": {{ "target": "ES2022", "lib": [{lib}], "strict": true, "noEmit": true }} }}"#
            ),
            &[(
                "index.ts",
                "const el = document.body;\nconst keep = [el];\n",
            )],
        );
        let (code, stdout, stderr) = fixture.check(&[]);
        assert_eq!(code, 0, "lib {lib}: {stdout}\n{stderr}");
    }
}

#[test]
fn no_lib_disables_the_bundled_snapshot() {
    let fixture = Fixture::new(
        "surge-standalone-nolib",
        r#"{ "compilerOptions": { "target": "ES2022", "noLib": true, "strict": true, "noEmit": true } }"#,
        &[("index.ts", "const xs: string[] = [];\nconst keep = xs;\n")],
    );

    let (code, stdout, _) = fixture.check(&[]);
    assert_eq!(code, 2, "expected noLib to remove the global types: {stdout}");
    assert!(
        stdout.contains("error TS2318") && stdout.contains("Cannot find global type 'Array'"),
        "expected tsc's missing-global-type errors under noLib, got: {stdout}"
    );
}

/// `/// <reference lib="..." />` between bundled files must be followed, so a
/// single seed pulls in its whole transitive graph.
#[test]
fn reference_lib_graph_resolves_within_the_snapshot() {
    let fixture = Fixture::new(
        "surge-standalone-refgraph",
        // `es2022` only references its predecessors; everything used below is
        // declared in a file reachable solely through that chain.
        r#"{ "compilerOptions": { "target": "ES2022", "lib": ["ES2022"], "strict": true, "noEmit": true } }"#,
        &[(
            "index.ts",
            r#"const map = new Map<string, number>();
const entries = [...map.entries()];
const flat = [[1], [2]].flat();
const trimmed = "  x  ".trimStart();
const big = 2n ** 3n;
const keep = [entries, flat, trimmed, big];
"#,
        )],
    );

    let (code, stdout, stderr) = fixture.check(&[]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
}

/// An override that cannot be honoured must say so and fall back to the bundled
/// snapshot rather than silently checking against nothing.
#[test]
fn missing_lib_path_override_warns_and_falls_back() {
    let fixture = Fixture::new(
        "surge-standalone-override",
        ES2022_TSCONFIG,
        &[("index.ts", "const xs: string[] = [];\nconst keep = xs;\n")],
    );
    let missing = fixture.root.join("no-such-lib-dir");

    let (code, stdout, stderr) = fixture.check(&["--typescript-lib-path", missing.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        stderr.contains("--typescript-lib-path") && stderr.contains("bundled TypeScript"),
        "expected a fallback warning naming the override, got: {stderr}"
    );
}

/// Requesting the project's installed TypeScript when there is none must warn
/// and still check successfully against the bundled snapshot.
#[test]
fn physical_libs_request_without_typescript_warns_and_falls_back() {
    let fixture = Fixture::new(
        "surge-standalone-physical",
        ES2022_TSCONFIG,
        &[("index.ts", "const xs: string[] = [];\nconst keep = xs;\n")],
    );

    let (code, stdout, stderr) = fixture.check(&["--physicalLibs"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        stderr.contains("--physicalLibs") && stderr.contains("bundled TypeScript"),
        "expected a fallback warning, got: {stderr}"
    );
}

/// Diagnostics that point into a bundled declaration must name the virtual
/// directory, not a path that looks like it exists on disk.
#[test]
fn bundled_declarations_use_a_virtual_path() {
    assert_eq!(surge_ts_checker::lowlevel::EMBEDDED_LIB_DIR, "<surge-lib>");

    let inputs = surge_ts_checker::lowlevel::load_generated_default_lib_inputs(false, None);
    assert!(!inputs.is_empty(), "bundled snapshot loaded no files");
    for input in &inputs {
        assert!(
            input.file_name.starts_with("<surge-lib>/lib.")
                && input.file_name.ends_with(".d.ts"),
            "unexpected bundled file identity: {}",
            input.file_name
        );
        assert!(
            !Path::new(&input.file_name).exists(),
            "bundled identity collides with a real file: {}",
            input.file_name
        );
    }
}
