//! Module-resolution parity fixtures.
//!
//! Each test pins a TypeScript resolver rule that surge must match; the
//! expected outcomes were verified against the workspace-local tsc (7.0.2).
//! See `crates/surge-ts/MODULE_RESOLUTION.md` for the rule inventory.

use std::{fs, path::PathBuf, process::Command, time::SystemTime};

use serde_json::Value;

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

fn run_cli_json(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_surge"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        matches!(output.status.code(), Some(0) | Some(2)),
        "surge exited with unexpected status {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(&String::from_utf8(output.stdout).unwrap()).unwrap()
}

fn check_project(root: &PathBuf) -> Value {
    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    run_cli_json(&["--project", project.as_str(), "--format", "json"])
}

fn diagnostic_codes(parsed: &Value) -> Vec<String> {
    parsed["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnostic| diagnostic["code"].as_str().unwrap().to_string())
        .collect()
}

const BUNDLER_TSCONFIG: &str = r#"
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "module": "preserve",
    "strict": true,
    "noEmit": true
  },
  "include": ["src/**/*"]
}
"#;

// tsc: `import "./user.js"` never resolves `user/index.ts` — an explicit
// runtime extension is a file-shaped path, not a directory lookup.
#[test]
fn explicit_js_never_resolves_directory_index() {
    let root = temp_dir("mr-js-no-dir-index");
    write_file(&root, "tsconfig.json", BUNDLER_TSCONFIG);
    write_file(&root, "src/index.ts", r#"import { a } from "./user.js";"#);
    write_file(&root, "src/user/index.ts", "export const a = 1;");

    assert_eq!(diagnostic_codes(&check_project(&root)), vec!["TS2307"]);
}

#[test]
fn explicit_mjs_and_cjs_never_resolve_directory_index() {
    let root = temp_dir("mr-mjs-cjs-no-dir-index");
    write_file(&root, "tsconfig.json", BUNDLER_TSCONFIG);
    write_file(
        &root,
        "src/index.ts",
        r#"
import { a } from "./user.mjs";
import { b } from "./other.cjs";
"#,
    );
    write_file(&root, "src/user/index.mts", "export const a = 1;");
    write_file(&root, "src/other/index.cts", "export const b = 2;");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        vec!["TS2307", "TS2307"]
    );
}

// tsc substitution matrix: `.js` never reaches `.mts`/`.cts`, and an
// extensionless specifier never probes them either.
#[test]
fn js_and_extensionless_do_not_resolve_m_c_flavors() {
    let root = temp_dir("mr-js-not-mts");
    write_file(&root, "tsconfig.json", BUNDLER_TSCONFIG);
    write_file(
        &root,
        "src/index.ts",
        r#"
import { b } from "./m.js";
import { c } from "./c.js";
import { d } from "./m2";
"#,
    );
    write_file(&root, "src/m.mts", "export const b = 2;");
    write_file(&root, "src/c.cts", "export const c = 3;");
    write_file(&root, "src/m2.mts", "export const d = 4;");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        vec!["TS2307", "TS2307", "TS2307"]
    );
}

#[test]
fn mjs_resolves_mts_and_cjs_resolves_cts() {
    let root = temp_dir("mr-mjs-mts");
    write_file(&root, "tsconfig.json", BUNDLER_TSCONFIG);
    write_file(
        &root,
        "src/index.ts",
        r#"
import { b } from "./m.mjs";
import { c } from "./c.cjs";
const useB: number = b;
const useC: string = c;
"#,
    );
    write_file(&root, "src/m.mts", "export const b: number = 2;");
    write_file(&root, "src/c.cts", "export const c: string = \"x\";");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

// tsc resolves `./comp.jsx` through the default substitution set
// (`.ts`/`.tsx`/`.d.ts`), same as `.js`.
#[test]
fn jsx_specifier_resolves_tsx_source() {
    let root = temp_dir("mr-jsx-tsx");
    write_file(
        &root,
        "tsconfig.json",
        r#"
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "module": "preserve",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true
  },
  "include": ["src/**/*"]
}
"#,
    );
    write_file(&root, "src/index.ts", r#"import { c } from "./comp.jsx";"#);
    write_file(&root, "src/comp.tsx", "export const c = 3;");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

#[test]
fn bundler_extensionless_relative_resolves() {
    let root = temp_dir("mr-bundler-extensionless");
    write_file(&root, "tsconfig.json", BUNDLER_TSCONFIG);
    write_file(
        &root,
        "src/index.ts",
        r#"
import { a } from "./file";
import { b } from "./dir";
"#,
    );
    write_file(&root, "src/file.ts", "export const a = 1;");
    write_file(&root, "src/dir/index.ts", "export const b = 2;");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

fn paths_tsconfig(paths: &str) -> String {
    format!(
        r#"
{{
  "compilerOptions": {{
    "moduleResolution": "bundler",
    "module": "preserve",
    "strict": true,
    "noEmit": true,
    "paths": {paths}
  }},
  "include": ["src/**/*"]
}}
"#
    )
}

// The wildcard pattern with the longest literal prefix wins, regardless of
// config order. If `@/*` won here, `value` would import the fallback module
// that does not export it.
#[test]
fn paths_longest_prefix_wins() {
    let root = temp_dir("mr-paths-longest-prefix");
    write_file(
        &root,
        "tsconfig.json",
        &paths_tsconfig(
            r#"{
      "@/*": ["./src/fallback/*"],
      "@/core/*": ["./src/core/*"]
    }"#,
        ),
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
import { value } from "@/core/value";
const use: number = value;
"#,
    );
    write_file(
        &root,
        "src/core/value.ts",
        "export const value: number = 1;",
    );
    write_file(&root, "src/fallback/value.ts", "export const notValue = 0;");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

#[test]
fn paths_exact_pattern_beats_wildcard() {
    let root = temp_dir("mr-paths-exact");
    write_file(
        &root,
        "tsconfig.json",
        &paths_tsconfig(
            r#"{
      "lib/special": ["./src/special.ts"],
      "lib/*": ["./src/generic/*"]
    }"#,
        ),
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
import { special } from "lib/special";
const use: string = special;
"#,
    );
    write_file(
        &root,
        "src/special.ts",
        "export const special: string = \"s\";",
    );
    write_file(&root, "src/generic/special.ts", "export const wrong = 0;");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

#[test]
fn paths_first_target_missing_second_succeeds() {
    let root = temp_dir("mr-paths-second-target");
    write_file(
        &root,
        "tsconfig.json",
        &paths_tsconfig(r#"{ "multi/*": ["./src/missing/*", "./src/real/*"] }"#),
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
import { second } from "multi/second";
const use: number = second;
"#,
    );
    write_file(
        &root,
        "src/real/second.ts",
        "export const second: number = 2;",
    );

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

// tsc resolves targets without a leading `./` against the mapping base (it
// additionally flags the config with TS5090 under TS7 — a config-level
// diagnostic surge does not model — but resolution still succeeds).
#[test]
fn paths_target_without_dot_slash_resolves() {
    let root = temp_dir("mr-paths-no-dot-slash");
    write_file(
        &root,
        "tsconfig.json",
        &paths_tsconfig(r#"{ "nodot/*": ["src/core/*"] }"#),
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
import { nd } from "nodot/nd";
const use: number = nd;
"#,
    );
    write_file(&root, "src/core/nd.ts", "export const nd: number = 3;");

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

const PKG_TSCONFIG: &str = r#"
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "module": "preserve",
    "strict": true,
    "noEmit": true
  },
  "include": ["packages/**/*.ts"]
}
"#;

// Two importers resolve the same bare specifier to different files through
// their own nested `node_modules`. A specifier-keyed resolution map would let
// the first importer's result leak into the second.
#[test]
fn same_package_name_different_importers() {
    let root = temp_dir("mr-nested-node-modules");
    write_file(&root, "tsconfig.json", PKG_TSCONFIG);
    write_file(
        &root,
        "packages/a/node_modules/dep/package.json",
        r#"{ "name": "dep", "types": "./index.d.ts" }"#,
    );
    write_file(
        &root,
        "packages/a/node_modules/dep/index.d.ts",
        "export declare const value: string;",
    );
    write_file(
        &root,
        "packages/b/node_modules/dep/package.json",
        r#"{ "name": "dep", "types": "./index.d.ts" }"#,
    );
    write_file(
        &root,
        "packages/b/node_modules/dep/index.d.ts",
        "export declare const value: number;",
    );
    write_file(
        &root,
        "packages/a/src/index.ts",
        r#"
import { value } from "dep";
const useA: string = value;
"#,
    );
    write_file(
        &root,
        "packages/b/src/index.ts",
        r#"
import { value } from "dep";
const useB: number = value;
"#,
    );

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

// `#alias` imports resolve against the importer's nearest enclosing package
// scope; the same alias in two scopes must not collide.
#[test]
fn package_imports_nearest_scope() {
    let root = temp_dir("mr-imports-scope");
    write_file(&root, "tsconfig.json", PKG_TSCONFIG);
    write_file(
        &root,
        "packages/a/package.json",
        r##"{ "name": "a", "imports": { "#util": "./src/util.ts" } }"##,
    );
    write_file(
        &root,
        "packages/b/package.json",
        r##"{ "name": "b", "imports": { "#util": "./src/util.ts" } }"##,
    );
    write_file(
        &root,
        "packages/a/src/util.ts",
        "export const util: string = \"a\";",
    );
    write_file(
        &root,
        "packages/b/src/util.ts",
        "export const util: number = 2;",
    );
    write_file(
        &root,
        "packages/a/src/index.ts",
        r##"
import { util } from "#util";
const useA: string = util;
"##,
    );
    write_file(
        &root,
        "packages/b/src/index.ts",
        r##"
import { util } from "#util";
const useB: number = util;
"##,
    );

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

// A package importing its own name resolves through its own `exports` map.
#[test]
fn package_self_name_import() {
    let root = temp_dir("mr-self-name");
    write_file(
        &root,
        "tsconfig.json",
        r#"
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "module": "preserve",
    "strict": true,
    "noEmit": true
  },
  "include": ["src/**/*.ts"]
}
"#,
    );
    write_file(
        &root,
        "package.json",
        r#"{ "name": "self-pkg", "exports": { ".": { "types": "./types/index.d.ts" } } }"#,
    );
    write_file(
        &root,
        "types/index.d.ts",
        "export declare const marker: string;",
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
import { marker } from "self-pkg";
const use: string = marker;
"#,
    );

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

// An `exports` map is authoritative: a `null` target blocks the subpath with
// no filesystem fallback.
#[test]
fn package_exports_null_blocks_subpath() {
    let root = temp_dir("mr-exports-null");
    write_file(&root, "tsconfig.json", BUNDLER_TSCONFIG);
    write_file(
        &root,
        "node_modules/pkg/package.json",
        r#"{ "name": "pkg", "exports": { ".": { "types": "./index.d.ts" }, "./blocked": null } }"#,
    );
    write_file(
        &root,
        "node_modules/pkg/index.d.ts",
        "export declare const root: number;",
    );
    write_file(
        &root,
        "node_modules/pkg/blocked.d.ts",
        "export declare const blocked: number;",
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
import { root } from "pkg";
import { blocked } from "pkg/blocked";
"#,
    );

    assert_eq!(diagnostic_codes(&check_project(&root)), vec!["TS2307"]);
}

#[test]
fn package_exports_exact_beats_pattern() {
    let root = temp_dir("mr-exports-exact");
    write_file(&root, "tsconfig.json", BUNDLER_TSCONFIG);
    write_file(
        &root,
        "node_modules/pkg/package.json",
        r#"{
  "name": "pkg",
  "exports": {
    "./features/*": { "types": "./dist/features/*.d.ts" },
    "./features/special": { "types": "./dist/special.d.ts" }
  }
}"#,
    );
    write_file(
        &root,
        "node_modules/pkg/dist/features/auth.d.ts",
        "export declare const auth: number;",
    );
    write_file(
        &root,
        "node_modules/pkg/dist/special.d.ts",
        "export declare const special: string;",
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
import { auth } from "pkg/features/auth";
import { special } from "pkg/features/special";
const useAuth: number = auth;
const useSpecial: string = special;
"#,
    );

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

// Under node16, an `.mts` importer selects the `import` condition and a
// `.cts` importer selects `require` from the same package `exports` map.
#[test]
fn node16_importer_flavor_selects_export_condition() {
    let root = temp_dir("mr-node16-conditions");
    write_file(
        &root,
        "tsconfig.json",
        r#"
{
  "compilerOptions": {
    "moduleResolution": "node16",
    "module": "node16",
    "strict": true,
    "noEmit": true
  },
  "include": ["src/**/*"]
}
"#,
    );
    write_file(
        &root,
        "node_modules/pkg/package.json",
        r#"{
  "name": "pkg",
  "exports": {
    ".": {
      "import": { "types": "./import.d.mts" },
      "require": { "types": "./require.d.cts" }
    }
  }
}"#,
    );
    write_file(
        &root,
        "node_modules/pkg/import.d.mts",
        "export declare const flavor: \"esm\";",
    );
    write_file(
        &root,
        "node_modules/pkg/require.d.cts",
        "export declare const flavor: \"cjs\";",
    );
    write_file(
        &root,
        "src/a.mts",
        "import { flavor } from \"pkg\";\nconst f: \"esm\" = flavor;\n",
    );
    write_file(
        &root,
        "src/b.cts",
        "import { flavor } from \"pkg\";\nconst f: \"cjs\" = flavor;\n",
    );

    assert_eq!(
        diagnostic_codes(&check_project(&root)),
        Vec::<String>::new()
    );
}

// Repeated runs over the same project must produce byte-identical JSON output
// (deterministic resolution and diagnostic ordering).
#[test]
fn repeated_runs_are_deterministic() {
    let root = temp_dir("mr-determinism");
    write_file(&root, "tsconfig.json", PKG_TSCONFIG);
    write_file(
        &root,
        "packages/a/node_modules/dep/package.json",
        r#"{ "name": "dep", "types": "./index.d.ts" }"#,
    );
    write_file(
        &root,
        "packages/a/node_modules/dep/index.d.ts",
        "export declare const value: string;",
    );
    write_file(
        &root,
        "packages/b/node_modules/dep/package.json",
        r#"{ "name": "dep", "types": "./index.d.ts" }"#,
    );
    write_file(
        &root,
        "packages/b/node_modules/dep/index.d.ts",
        "export declare const value: number;",
    );
    write_file(
        &root,
        "packages/a/src/index.ts",
        "import { value } from \"dep\";\nconst wrong: number = value;\n",
    );
    write_file(
        &root,
        "packages/b/src/index.ts",
        "import { value } from \"dep\";\nconst wrong: string = value;\n",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let args = ["--project", project.as_str(), "--format", "json"];

    let first = Command::new(env!("CARGO_BIN_EXE_surge"))
        .args(args)
        .output()
        .unwrap();
    for _ in 0..3 {
        let next = Command::new(env!("CARGO_BIN_EXE_surge"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(first.stdout, next.stdout);
    }

    // Both importers keep their own (wrong) assignment: exactly two TS2322s.
    let parsed: Value = serde_json::from_str(std::str::from_utf8(&first.stdout).unwrap()).unwrap();
    assert_eq!(diagnostic_codes(&parsed), vec!["TS2322", "TS2322"]);
}
