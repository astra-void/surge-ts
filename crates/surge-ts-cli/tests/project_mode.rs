use std::{fs, path::PathBuf, process::Command, time::SystemTime};

use serde_json::Value;
use surge_ts_checker::{CheckerOptions, check_source_with_options};
use surge_ts_config::{TsConfigLoadOptions, load_tsconfig};

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
    let output = Command::new(env!("CARGO_BIN_EXE_surge"))
        .args(args)
        .output()
        .unwrap();

    // surge mirrors tsc's exit codes: 0 when clean, 2 when diagnostics were
    // reported. Both are normal completions for these tests; only an unexpected
    // status (a panic/abort, or a config/usage error) is a failure.
    assert!(
        matches!(output.status.code(), Some(0) | Some(2)),
        "surge exited with unexpected status {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
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
    Command::new(env!("CARGO_BIN_EXE_surge"))
        .args(args)
        .output()
        .unwrap()
}

fn workspace_root() -> PathBuf {
    fs::canonicalize(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".."),
    )
    .unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    })
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

fn json_diagnostic_fingerprints(parsed: &Value) -> Vec<String> {
    json_diagnostics(parsed)
        .iter()
        .map(|diagnostic| {
            let file_name = diagnostic["fileName"].as_str().unwrap_or("");
            let code = diagnostic["code"].as_str().unwrap_or("");
            let line = diagnostic["line"]
                .as_u64()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string());
            let column = diagnostic["column"]
                .as_u64()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string());
            let message = diagnostic["message"].as_str().unwrap_or("");
            format!("{file_name}|{code}|{line}|{column}|{message}")
        })
        .collect()
}

#[test]
fn project_mode_maps_strict_to_no_implicit_any() {
    let workspace_root = workspace_root();
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
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            types: Vec::new(),
            stub_external_modules: false,
            no_implicit_any: loaded.compiler_options.no_implicit_any,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unused_locals: false,
            no_unused_parameters: false,
        },
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.to_string() == "TS7006")
    );
}

#[test]
fn project_mode_generated_default_libs_visible_by_default() {
    let root = temp_dir("project-default-lib-visible");
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
        r#"
        const n = Math.max(1, 2);
        const transport: AuthenticatorTransport = "usb";
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert!(json_diagnostics(&parsed).is_empty());
}

#[test]
fn project_mode_lib_option_es_only_skips_dom_generated_libs() {
    let root = temp_dir("project-lib-es-only");
    write_file(
        &root,
        "tsconfig.json",
        r#"
        {
          "compilerOptions": {
            "lib": ["ES2022"]
          },
          "include": ["src/**/*.ts"]
        }
        "#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        const n = Math.max(1, 2);
        const transport: AuthenticatorTransport = "usb";
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert_eq!(json_diagnostic_codes(&parsed), vec!["TS2304"]);
}

/// Physical lib loading requires the `typescript` package to be installed
/// (`pnpm install`). `cargo test` must not depend on that, so physical-lib
/// tests skip when the package is absent.
fn typescript_lib_available() -> bool {
    workspace_root()
        .join("node_modules/typescript/lib/lib.es5.d.ts")
        .is_file()
}

fn run_physical_fixture_codes(fixture: &str) -> Vec<String> {
    let project = compat_project_root(fixture).join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--physicalLibs",
    ]);
    json_diagnostic_codes(&parsed)
}

#[test]
fn project_mode_physical_libs_resolve_array_callback_return() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `values.map(v => v.toString())` must infer `string[]`, so assigning it to
    // `number[]` is the only error, mirroring tsc against the real lib.es5.
    assert_eq!(
        run_physical_fixture_codes("physical-lib-es-array-basic"),
        vec!["TS2322"]
    );
}

#[test]
fn project_mode_physical_libs_resolve_map_generic_methods() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    assert_eq!(
        run_physical_fixture_codes("physical-lib-es-map-set-basic"),
        vec!["TS2322"]
    );
}

#[test]
fn project_mode_physical_libs_resolve_index_signature() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    assert_eq!(
        run_physical_fixture_codes("physical-lib-index-signature-basic"),
        vec!["TS2322"]
    );
}

#[test]
fn project_mode_physical_libs_new_promise_void_executor() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // A `Promise<void>` executor (contextual or explicit `<void>`) may call
    // `resolve()` with no argument: the constructor infers `T = void` from the
    // expected type, so the executor's `resolve: (value: void | PromiseLike<void>)`
    // parameter is optional. `new Promise<number>(r => r(5))` stays valid too.
    // tsc reports nothing here.
    assert_eq!(
        run_physical_fixture_codes("physical-lib-new-promise-executor-basic"),
        Vec::<String>::new()
    );
}

#[test]
fn project_mode_physical_libs_required_omit_pick() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `Required<Omit<T, K>> & Pick<T, K>` (ky's `InternalRetryOptions`) must
    // resolve: `Required` makes each property required while keeping an explicit
    // `| undefined` member, so the object literal is assignable. tsc reports
    // nothing. Regression for the spurious TS2353 ('limit' missing).
    assert_eq!(
        run_physical_fixture_codes("physical-lib-required-omit-pick-basic"),
        Vec::<String>::new()
    );
}

#[test]
fn project_mode_nested_namespace_member_resolves_siblings_on_lazy_peel() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // A generic alias indexing a nested-namespace interface
    // (`T extends keyof Inner.Table ? Inner.Table[T] : never`, React's
    // `ComponentProps<"button">` shape) finds the interface through its bare
    // dual-registration key, which carries no resolution scope of its own. The
    // lazy peel must reinstall the scope active where the reference was created,
    // or every member referencing an outer sibling (`MyLib.Payload<string>`)
    // degrades to `unknown` — losing the TS2322 below and implicit-any'ing the
    // contextual callback. tsc reports exactly the one TS2322.
    assert_eq!(
        run_physical_fixture_codes("namespace-nested-member-lazy-scope-basic"),
        vec!["TS2322"]
    );
}

#[test]
fn project_mode_function_type_binding_pattern_parameter() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // A function-type parameter written as a destructuring pattern
    // (`render: ({ field }: { field: T }) => string`, react-hook-form's
    // `ControllerProps.render` shape) must parse: failing it degrades the whole
    // function type — and any intersection containing it — to `unknown`,
    // implicit-any'ing the render callback's bindings. Only the deliberate
    // `Controller`-to-number probe reports.
    assert_eq!(
        run_physical_fixture_codes("function-type-binding-pattern-param-basic"),
        vec!["TS2322"]
    );
}

#[test]
fn project_mode_interface_extends_inherits_call_signature() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // A call signature declared on a base interface (React's
    // `ForwardRefExoticComponent extends ExoticComponent` shape) must survive
    // the extends merge, or `T extends (props: infer P) => unknown` cannot
    // recover the props type from the component value. tsc reports exactly the
    // one TS2322 probe.
    assert_eq!(
        run_physical_fixture_codes("interface-extends-call-signature-basic"),
        vec!["TS2322"]
    );
}

#[test]
fn project_mode_physical_libs_no_lib_disables_globals() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `noLib: true` disables physical default libs too, so `Promise`/`Date` are
    // missing (matching tsc's low-cascade missing-global behaviour).
    let codes = run_physical_fixture_codes("physical-lib-no-lib-basic");
    assert!(!codes.is_empty());
    // Missing global types under noLib (`Cannot find global type 'Array'`, etc.).
    assert!(codes.iter().all(|code| code == "TS2318"));
}

/// Run a compat fixture in default mode (no `--physicalLibs`). Physical lib
/// loading is the default, so these prove the real lib graph loads without the
/// opt-in flag.
fn run_default_fixture_codes(fixture: &str) -> Vec<String> {
    let project = compat_project_root(fixture).join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    json_diagnostic_codes(&parsed)
}

#[test]
fn project_mode_physical_libs_are_the_default_es_dom() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // No `--physicalLibs`, no `compilerOptions.lib`: the target's `.full` lib
    // graph (ES + DOM) loads by default, so `Map`, `Promise`, `JSON`, `Number`
    // and `Array.from` all resolve.
    assert!(run_default_fixture_codes("default-lib-physical-default-es-dom-basic").is_empty());
}

#[test]
fn project_mode_physical_libs_target_selects_graph() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `target: es2017` (no explicit lib) seeds `lib.es2017.full`, so the es2017
    // additions `Object.entries`/`Object.values`/`String.padStart` resolve.
    assert!(run_default_fixture_codes("default-lib-physical-target-graph-basic").is_empty());
}

#[test]
fn project_mode_physical_libs_lib_option_dom() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    assert!(run_default_fixture_codes("default-lib-physical-lib-option-dom-basic").is_empty());
}

#[test]
fn project_mode_physical_libs_default_no_lib_disables_globals() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `noLib: true` is honored even though physical loading is the default.
    let codes = run_default_fixture_codes("default-lib-physical-no-lib-basic");
    assert!(!codes.is_empty());
    assert!(codes.iter().all(|code| code == "TS2318"));
}

#[test]
fn project_mode_physical_libs_dom_globals() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `navigator`, `document`, `window` come from the real DOM lib.
    assert!(run_default_fixture_codes("default-lib-physical-dom-globals-basic").is_empty());
}

#[test]
fn project_mode_physical_libs_timers() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `setInterval`/`clearInterval`/`setTimeout`/`clearTimeout` come from the lib.
    assert!(run_default_fixture_codes("default-lib-physical-timers-basic").is_empty());
}

#[test]
fn project_mode_physical_libs_formdata() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    assert!(run_default_fixture_codes("default-lib-physical-formdata-basic").is_empty());
}

#[test]
fn project_mode_physical_libs_html_element() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    // `HTMLDivElement` and `HTMLElement` come from the real DOM lib, not a
    // hardcoded global.
    assert!(run_default_fixture_codes("default-lib-physical-html-element-basic").is_empty());
}

#[test]
fn project_mode_lib_option_dom_enables_authenticator_transport() {
    let root = temp_dir("project-lib-dom");
    write_file(
        &root,
        "tsconfig.json",
        r#"
        {
          "compilerOptions": {
            "lib": ["ES2022", "DOM"]
          },
          "include": ["src/**/*.ts"]
        }
        "#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        const n = Math.max(1, 2);
        const transport: AuthenticatorTransport = "usb";
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert!(json_diagnostics(&parsed).is_empty());
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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

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
    write_file(&root, "src/a.ts", "let greeting = \"Ada\";");
    write_file(&root, "src/b.ts", "let value: string = greeting;");

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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(!stdout.contains("src/b.ts\nerror[surge::parser-error]"));
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
    // Default tsc-compatible output prints nothing on success (like `tsc`).
    assert!(stdout.trim().is_empty());
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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("surge::parser-error"));
    assert!(!stdout.contains("src/b.ts\nerror[surge::parser-error]"));
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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--diagnosticProfile",
        "native",
    ]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("src/a.ts"));
    assert!(stdout.contains("surge::parser-error"));
    assert!(!stdout.contains("src/b.ts\nerror[surge::parser-error]"));
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
    // Default tsc-compatible output prints nothing on success (like `tsc`).
    assert!(stdout.trim().is_empty());
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
fn cli_compat_report_includes_build_info() {
    let project = compat_project_root("package-imports").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(
        parsed["buildInfo"]["packageVersion"],
        Value::String(env!("CARGO_PKG_VERSION").to_string())
    );
    assert_eq!(
        parsed["buildInfo"]["buildProfile"],
        Value::String(option_env!("PROFILE").unwrap_or("unknown").to_string())
    );
    assert!(parsed["buildInfo"]["binaryPath"].is_string());
    assert!(parsed["buildInfo"]["currentDir"].is_string());
    assert!(parsed["buildInfo"]["workspaceRoot"].is_string());
}

#[test]
fn cli_project_reports_zero_source_files_explicitly() {
    let root = temp_dir("project-no-source-files");
    write_file(&root, "tsconfig.json", r#"{ "include": ["src"] }"#);

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(
        codes,
        vec!["surge::project-has-no-source-files".to_string()]
    );
}

#[test]
fn cli_compat_report_reports_visibility_warning_when_no_sources_load() {
    let root = temp_dir("project-no-source-files-report");
    write_file(&root, "tsconfig.json", r#"{ "include": ["src"] }"#);

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["filesLoaded"], Value::from(0));
    assert_eq!(
        parsed["visibilityWarning"],
        Value::String("no source files were discovered for the project".to_string())
    );
    assert_eq!(
        parsed["byCode"][0]["code"],
        Value::String("surge::project-has-no-source-files".to_string())
    );
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
    let (stdout, stderr) = run_cli(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--diagnosticProfile",
        "native",
    ]);

    assert!(stderr.is_empty());
    assert!(stdout.contains("Parser errors: 1"));
    assert!(stdout.contains("surge::parser-error"));
}

#[test]
fn cli_project_tsc_profile_suppresses_custom_checker_diagnostics() {
    let root = temp_dir("project-tsc-profile-custom-suppression");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "import { User from \"./user\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let tsc = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    assert!(json_diagnostic_codes(&tsc).is_empty());

    let native = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--diagnosticProfile",
        "native",
    ]);
    assert!(
        json_diagnostic_codes(&native)
            .iter()
            .any(|code| code.starts_with("surge::"))
    );
}

#[test]
fn cli_project_jobs_accept_one_two_and_four() {
    let project = compat_project_root("parallel-ordering-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    for jobs in ["1", "2", "4"] {
        let json = run_cli_json(&[
            "--project",
            project.as_str(),
            "--format",
            "json",
            "--jobs",
            jobs,
        ]);
        assert!(!json_diagnostics(&json).is_empty());
    }
}

#[test]
fn cli_project_jobs_reject_zero_and_non_numeric() {
    let project = compat_project_root("parallel-ordering-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let zero = run_cli_raw(&["--project", project.as_str(), "--jobs", "0"]);
    assert!(!zero.status.success());
    let zero_stderr = String::from_utf8(zero.stderr).unwrap();
    assert!(zero_stderr.contains("--jobs must be greater than 0"));

    let invalid = run_cli_raw(&["--project", project.as_str(), "--jobs", "not-a-number"]);
    assert!(!invalid.status.success());
    let invalid_stderr = String::from_utf8(invalid.stderr).unwrap();
    assert!(invalid_stderr.contains("invalid value for --jobs"));
}

#[test]
fn cli_project_jobs_match_serial_json_diagnostics() {
    let project = compat_project_root("parallel-ordering-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let serial = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let jobs1 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "1",
    ]);
    let jobs4 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "4",
    ]);

    let serial_fingerprints = json_diagnostic_fingerprints(&serial);
    let jobs1_fingerprints = json_diagnostic_fingerprints(&jobs1);
    let jobs4_fingerprints = json_diagnostic_fingerprints(&jobs4);

    assert_eq!(serial_fingerprints, jobs1_fingerprints);
    assert_eq!(jobs1_fingerprints, jobs4_fingerprints);
}

#[test]
fn cli_project_jobs_keep_native_profile_opt_in() {
    let root = temp_dir("project-jobs-native-profile");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(&root, "src/a.ts", "import { User from \"./user\";");

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let native = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--diagnosticProfile",
        "native",
        "--jobs",
        "4",
    ]);

    assert!(
        json_diagnostic_codes(&native)
            .iter()
            .any(|code| code.starts_with("surge::"))
    );
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
fn cli_project_file_discovery_fixture_loads_all_supported_extensions() {
    let project = compat_project_root("project-file-discovery-extensions").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert_eq!(codes.iter().filter(|code| *code == "TS2322").count(), 4);
    assert!(!codes.contains(&"surge::unsupported-declaration".to_string()));
}

#[test]
fn cli_project_file_discovery_fixture_compat_report_counts_loaded_files() {
    let project = compat_project_root("project-file-discovery-extensions").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["filesLoaded"], Value::from(5));
    assert!(parsed.get("visibilityWarning").is_none());
}

#[test]
fn cli_import_graph_generated_relative_basic_fixture_loads_relative_candidates() {
    let project =
        compat_project_root("import-graph-generated-relative-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert_eq!(codes.iter().filter(|code| *code == "TS2307").count(), 1);
}

#[test]
fn cli_import_graph_generated_relative_basic_fixture_compat_report_loads_files() {
    let project =
        compat_project_root("import-graph-generated-relative-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["loadedSourceFiles"], Value::from(3));
    assert_eq!(parsed["diagnosticsTotal"], Value::from(1));
}

#[test]
fn cli_paths_wildcard_import_graph_basic_fixture_resolves_relative_alias_target() {
    let project = compat_project_root("paths-wildcard-import-graph-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert_eq!(codes.iter().filter(|code| *code == "TS2307").count(), 1);
}

#[test]
fn cli_paths_wildcard_import_graph_basic_fixture_compat_report_loads_files() {
    let project = compat_project_root("paths-wildcard-import-graph-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["loadedSourceFiles"], Value::from(2));
    assert_eq!(parsed["diagnosticsTotal"], Value::from(1));
}

#[test]
fn cli_import_graph_dependency_js_not_source_fixture_uses_declaration_not_js() {
    let project =
        compat_project_root("import-graph-dependency-js-not-source").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_import_graph_dependency_js_not_source_fixture_compat_report_tracks_dependency_js_zero() {
    let project =
        compat_project_root("import-graph-dependency-js-not-source").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["filesLoaded"], Value::from(1));
    assert_eq!(parsed["loadedSourceFiles"], Value::from(1));
    assert_eq!(parsed["loadedDependencyDeclarationFiles"], Value::from(1));
    assert_eq!(parsed["diagnosticsTotal"], Value::from(0));
}

#[test]
fn cli_builtin_visibility_project_graph_basic_fixture_keeps_synthetic_builtins_visible() {
    let project =
        compat_project_root("builtin-visibility-project-graph-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_builtin_visibility_project_graph_basic_fixture_compat_report_tracks_loaded_imported_file() {
    let project =
        compat_project_root("builtin-visibility-project-graph-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["filesLoaded"], Value::from(2));
    assert_eq!(parsed["loadedSourceFiles"], Value::from(2));
    assert_eq!(parsed["diagnosticsTotal"], Value::from(0));
}

#[test]
fn cli_builtin_visibility_import_graph_basic_fixture_keeps_synthetic_builtins_visible() {
    let project =
        compat_project_root("builtin-visibility-import-graph-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_builtin_visibility_import_graph_basic_fixture_compat_report_tracks_loaded_imported_file() {
    let project =
        compat_project_root("builtin-visibility-import-graph-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["filesLoaded"], Value::from(2));
    assert_eq!(parsed["loadedSourceFiles"], Value::from(2));
    assert_eq!(parsed["diagnosticsTotal"], Value::from(0));
}

#[test]
fn cli_builtin_visibility_function_body_basic_fixture_keeps_synthetic_builtins_visible() {
    let project =
        compat_project_root("builtin-visibility-function-body-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_builtin_visibility_function_body_basic_fixture_is_stable_across_jobs() {
    let project =
        compat_project_root("builtin-visibility-function-body-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let jobs1 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "1",
    ]);
    let jobs4 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "4",
    ]);

    assert_eq!(
        json_diagnostic_fingerprints(&jobs1),
        json_diagnostic_fingerprints(&jobs4)
    );
}

#[test]
fn cli_module_local_functions_basic_fixture_keeps_same_file_helpers_visible() {
    let project = compat_project_root("module-local-functions-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_default_parameter_inference_basic_fixture_keeps_defaults_typed() {
    let project = compat_project_root("default-parameter-inference-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert_eq!(codes.iter().filter(|code| *code == "TS7006").count(), 1);
}

#[test]
fn cli_function_body_scope_hardening_fixture_keeps_locals_visible() {
    let project = compat_project_root("function-body-scope-hardening").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_function_body_scope_hardening_fixture_is_stable_across_jobs() {
    let project = compat_project_root("function-body-scope-hardening").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let jobs1 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "1",
    ]);
    let jobs4 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "4",
    ]);

    assert_eq!(
        json_diagnostic_fingerprints(&jobs1),
        json_diagnostic_fingerprints(&jobs4)
    );
}

#[test]
fn cli_module_local_helper_functions_hardening_fixture_keeps_helpers_visible() {
    let project =
        compat_project_root("module-local-helper-functions-hardening").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_module_local_helper_functions_hardening_fixture_is_stable_across_jobs() {
    let project =
        compat_project_root("module-local-helper-functions-hardening").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let jobs1 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "1",
    ]);
    let jobs4 = run_cli_json(&[
        "--project",
        project.as_str(),
        "--format",
        "json",
        "--jobs",
        "4",
    ]);

    assert_eq!(
        json_diagnostic_fingerprints(&jobs1),
        json_diagnostic_fingerprints(&jobs4)
    );
}

#[test]
fn cli_primitive_methods_basic_fixture_supports_string_number_and_array_methods() {
    let project = compat_project_root("primitive-methods-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_new_expression_builtins_basic_fixture_supports_builtin_constructors() {
    let project = compat_project_root("new-expression-builtins-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_object_shorthand_scope_basic_fixture_keeps_shorthand_locals_visible() {
    let project = compat_project_root("object-shorthand-scope-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_function_body_local_visibility_basic_fixture_keeps_locals_visible() {
    let project = compat_project_root("function-body-local-visibility-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert!(codes.is_empty());
}

#[test]
fn cli_dependency_incomplete_declaration_export_fallback_fixture_keeps_local_ts2305() {
    let project = compat_project_root("dependency-incomplete-declaration-export-fallback")
        .join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert_eq!(codes.iter().filter(|code| *code == "TS2305").count(), 2);
}

#[test]
fn cli_relative_directory_index_basic_fixture_resolves_loaded_directory_indexes() {
    let project = compat_project_root("relative-directory-index-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let codes = json_diagnostic_codes(&parsed);

    assert_eq!(codes.iter().filter(|code| *code == "TS2305").count(), 1);
    assert_eq!(codes.iter().filter(|code| *code == "TS2307").count(), 1);
    assert!(!codes.contains(&"surge::unsupported-module-syntax".to_string()));
}

#[test]
fn cli_tsx_parser_safe_basic_fixture_reports_ts2322() {
    let project = compat_project_root("tsx-parser-safe-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert_eq!(json_diagnostic_codes(&parsed), vec!["TS2322".to_string()]);
}

#[test]
fn cli_tsx_jsx_basic_fixture_reports_element_not_assignable() {
    let project = compat_project_root("tsx-jsx-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    // Only the meaningful assignment mismatch is reported; the well-formed JSX on
    // lines 2-3 produces no cascade. The conservative `JSX.Element` stand-in renders
    // as `Element`, matching tsc's message exactly.
    assert_eq!(json_diagnostic_codes(&parsed), vec!["TS2322".to_string()]);
    assert_eq!(json_diagnostic_lines(&parsed, "TS2322"), vec![Some(4)]);
    let message = json_diagnostics(&parsed)[0]["message"].as_str().unwrap();
    assert_eq!(
        message,
        "Type 'Element' is not assignable to type 'number'."
    );
}

#[test]
fn cli_jsx_runtime_module_namespace_fixture_types_intrinsic_callbacks() {
    let project = compat_project_root("jsx-runtime-module-namespace-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    // React-19 shape: no global `JSX`; the namespace lives at `React.JSX` inside
    // the react module. Under `jsx: react-jsx` an intrinsic tag in a file with no
    // `React` binding still resolves through the runtime declarer, so the
    // `onClick` arrow is contextually typed (TS2322 inside the body, no TS7006)
    // and an unknown tag still reports TS2339.
    assert_eq!(
        json_diagnostic_codes(&parsed),
        vec!["TS2322".to_string(), "TS2339".to_string()]
    );
    assert_eq!(json_diagnostic_lines(&parsed, "TS2322"), vec![Some(1)]);
    assert_eq!(json_diagnostic_lines(&parsed, "TS2339"), vec![Some(2)]);
}

#[test]
fn cli_jsx_imported_alias_props_fixture_types_component_callbacks() {
    let project = compat_project_root("jsx-imported-alias-props-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    // A component whose props type is a LOCAL alias of an imported qualified type
    // (`type ButtonProps = React.ButtonAttributes`): the alias must not bake a
    // degraded signature during the binding passes (its attached scope carries no
    // import layers — resolution falls back to the per-file module scope), so the
    // `onClick` arrow is contextually typed: TS2322 inside the body, no TS7006.
    assert_eq!(json_diagnostic_codes(&parsed), vec!["TS2322".to_string()]);
    assert_eq!(json_diagnostic_lines(&parsed, "TS2322"), vec![Some(9)]);
}

#[test]
fn cli_tsx_jsx_expression_diagnostics_basic_fixture_reports_unresolved_child() {
    let project = compat_project_root("tsx-jsx-expression-diagnostics-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    // The unresolved name inside `{missingValue}` is still reported; the resolved
    // `{ok}` container produces nothing.
    assert_eq!(json_diagnostic_codes(&parsed), vec!["TS2304".to_string()]);
    assert_eq!(json_diagnostic_lines(&parsed, "TS2304"), vec![Some(3)]);
}

#[test]
fn cli_tsx_jsx_attributes_basic_fixture_reports_unresolved_component() {
    let project = compat_project_root("tsx-jsx-attributes-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    // The capitalized `<Button />` tag is a value reference and reports TS2304 when
    // unresolved; the intrinsic `<div id="root" />` and the resolved `{count}`
    // attribute do not cascade.
    assert_eq!(json_diagnostic_codes(&parsed), vec!["TS2304".to_string()]);
    assert_eq!(json_diagnostic_lines(&parsed, "TS2304"), vec![Some(3)]);
}

#[test]
fn cli_tsx_generic_angle_regression_basic_fixture_has_no_diagnostics() {
    let project = compat_project_root("tsx-generic-angle-regression-basic").join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    // Pins that adding JSX parsing does not disturb `.ts` generic call / angle-bracket
    // behavior: both compilers agree on zero diagnostics.
    assert!(json_diagnostics(&parsed).is_empty());
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
    assert!(parsed["diagnosticsByFileKind"].is_array());
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
fn cli_compat_report_json_matches_plain_json_diagnostics_total() {
    let root = temp_dir("project-compat-report-parity");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        "export * from \"./models\";\nexport { Missing } from \"./models\";",
    );
    write_file(
        &root,
        "src/models/index.ts",
        "export interface User { name: string; }",
    );
    write_file(
        &root,
        "src/pages/index.ts",
        "import { User } from \"..\";\nexport const currentUser: User = { name: \"Ada\" };",
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();

    let plain = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    let report = run_cli_json(&[
        "--project",
        project.as_str(),
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(plain["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(report["diagnosticsTotal"], Value::from(1));
    assert_eq!(
        report["byCode"][0]["code"],
        Value::String("TS2305".to_string())
    );
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
    assert!(stdout.contains("Diagnostics: 0"));
    assert!(!stdout.contains("surge::unsupported-module-syntax"));
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
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_surge"));
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
    assert!(stdout.contains("TS2307"));
    assert!(stdout.contains("By code:"));

    let (stdout, _stderr) = run_cli(&[
        "--project",
        root.join("tsconfig.json").to_string_lossy().as_ref(),
        "--compatReport",
        "--stubExternalModules",
    ]);
    assert!(stdout.contains("External module stubs: 2"));
    assert!(!stdout.contains("error[TS2307]"));
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
    // Both `react` and `zustand` are non-relative references (total) and neither
    // resolves in this fixture, so both are unresolved and none resolved.
    assert_eq!(stubs.get("total").unwrap().as_u64().unwrap(), 2);
    assert_eq!(stubs.get("unresolved").unwrap().as_u64().unwrap(), 2);
    assert_eq!(stubs.get("resolved").unwrap().as_u64().unwrap(), 0);
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

#[test]
fn cli_package_declarations_resolves_subpath_d_ts_file() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    // "subpath-pkg/feature" is on line 5, it should be resolved
    assert!(!lines.contains(&Some(5)));
}

#[test]
fn cli_package_declarations_resolves_subpath_index_d_ts_fallback() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    // "subpath-pkg/nested/path" is on line 6, it remains unresolved in the current oracle.
    assert!(lines.contains(&Some(6)));
}

#[test]
fn cli_package_declarations_resolves_scoped_subpath() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    // "@scope/subtool/helpers" is on line 36, it should be resolved
    assert!(!lines.contains(&Some(36)));
}

#[test]
fn cli_package_declarations_ignores_wildcard_exports() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    // "exports-types-pkg/wild/wild" is on line 28, should NOT be resolved
    assert!(lines.contains(&Some(28)));
}

#[test]
fn cli_package_declarations_resolves_exports_types_subpath() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    // "exports-types-pkg/feature" is on line 14 and resolves.
    // "exports-types-pkg/nested/path" is on line 15 and remains unresolved in the current oracle.
    assert!(!lines.contains(&Some(14)));
    assert!(lines.contains(&Some(15)));
}

#[test]
fn cli_package_declarations_ignores_runtime_only_exports() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    // "exports-types-pkg/runtime-only" is on line 17, should NOT be resolved
    assert!(lines.contains(&Some(17)));
}

#[test]
fn cli_package_declarations_side_effect_resolved_subpath_no_ts2882() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let has_error = json_diagnostics(&parsed).iter().any(|d| {
        d["code"].as_str() == Some("TS2882")
            && d["fileName"].as_str().unwrap().contains("side-effect.ts")
            && d["line"].as_u64() == Some(2)
    });

    assert!(!has_error);
}

#[test]
fn cli_package_declarations_unresolved_subpath_reports_ts2307() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2307");
    // "pkg/subpath" in subpaths.ts is on line 1, should be unresolved
    assert!(lines.contains(&Some(1)));
}

#[test]
fn cli_package_declarations_unresolved_side_effect_subpath_reports_ts2882() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2882");
    // "exports-types-pkg/runtime-only" in side-effect.ts is on line 3, should be unresolved
    assert!(lines.contains(&Some(3)));
}

#[test]
fn cli_package_declarations_stub_external_modules_suppresses_unresolved_subpath_only() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--stubExternalModules",
        "--format",
        "json",
    ]);

    let lines_2307 = json_diagnostic_lines(&parsed, "TS2307");
    let lines_2882 = json_diagnostic_lines(&parsed, "TS2882");
    let codes = json_diagnostic_codes(&parsed);

    // Suppressed:
    assert!(!lines_2307.contains(&Some(1))); // pkg/subpath
    assert!(!lines_2307.contains(&Some(17))); // runtime-only
    assert!(!lines_2882.contains(&Some(3))); // runtime-only side-effect

    // Still semantic errors from resolved ones:
    // mismatch on line 10 TS2322 should still be there
    assert!(codes.contains(&"TS2322".to_string()));
}

#[test]
fn cli_package_declarations_missing_export_from_resolved_subpath_reports_ts2305() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-declarations/tsconfig.json",
        "--format",
        "json",
    ]);

    let lines = json_diagnostic_lines(&parsed, "TS2305");
    // We'll add an import in missing-export.ts line 2 that expects TS2305
    assert!(lines.contains(&Some(2)));
}

#[test]
fn cli_package_types_node_modules_basic_resolves_bundled_types() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-node-modules-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    assert!(json_diagnostic_codes(&parsed).is_empty());
}

#[test]
fn cli_package_types_exports_conditions_basic_resolves_nested_types() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-exports-conditions-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2305".to_string()));
    assert!(!codes.contains(&"TS2307".to_string()));
}

#[test]
fn cli_package_types_at_types_fallback_basic_resolves_root_package() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-at-types-fallback-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    assert!(json_diagnostic_codes(&parsed).is_empty());
}

#[test]
fn cli_package_types_scoped_at_types_fallback_basic_resolves_scoped_package() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-scoped-at-types-fallback-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    assert!(json_diagnostic_codes(&parsed).is_empty());
}

#[test]
fn cli_package_types_export_equals_import_require_valid_binds_value() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-export-equals-import-require-valid/tsconfig.json",
        "--format",
        "json",
    ]);

    assert!(
        json_diagnostic_codes(&parsed).is_empty(),
        "{:?}",
        json_diagnostic_codes(&parsed)
    );
}

#[test]
fn cli_package_types_export_equals_property_call_valid_resolves_method() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-export-equals-property-call-valid/tsconfig.json",
        "--format",
        "json",
    ]);

    assert!(
        json_diagnostic_codes(&parsed).is_empty(),
        "{:?}",
        json_diagnostic_codes(&parsed)
    );
}

#[test]
fn cli_package_types_export_equals_property_call_argument_mismatch_reports_ts2345() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-export-equals-property-call-argument-mismatch/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2345".to_string()), "{codes:?}");
    assert!(!codes.contains(&"TS2304".to_string()), "{codes:?}");
    assert!(json_diagnostic_lines(&parsed, "TS2345").contains(&Some(3)));
}

#[test]
fn cli_package_types_export_equals_missing_export_target_no_cascade() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-export-equals-missing-export-target-no-cascade/tsconfig.json",
        "--format",
        "json",
    ]);

    // The package's `export = missingValue` target is undefined; the consumer
    // binds an unknown value and must not cascade name/property errors.
    let codes = json_diagnostic_codes(&parsed);
    assert!(!codes.contains(&"TS2304".to_string()), "{codes:?}");
    assert!(!codes.contains(&"TS2339".to_string()), "{codes:?}");
}

#[test]
fn cli_package_types_import_require_missing_package_reports_ts2307() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-import-require-missing-package/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2307".to_string()), "{codes:?}");
    assert!(!codes.contains(&"TS2304".to_string()), "{codes:?}");
}

#[test]
fn cli_package_types_import_require_subpath_valid_binds_value() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-types-import-require-subpath-valid/tsconfig.json",
        "--format",
        "json",
    ]);

    assert!(
        json_diagnostic_codes(&parsed).is_empty(),
        "{:?}",
        json_diagnostic_codes(&parsed)
    );
}

/// Run a compat-project fixture and assert the diagnostic codes match the tsc
/// oracle exactly, with each code landing on the expected line. Oracle-backed:
/// the expectations mirror `tsc --noEmit` on the same fixture (TypeScript 6.0.3).
fn assert_fixture_codes_at_lines(fixture: &str, expected: &[(&str, u64)]) {
    let project = format!("../../tests/compat-projects/{fixture}/tsconfig.json");
    let parsed = run_cli_json(&["--project", &project, "--format", "json"]);

    let mut got: Vec<(String, Option<u64>)> = json_diagnostics(&parsed)
        .iter()
        .map(|d| (d["code"].as_str().unwrap().to_string(), d["line"].as_u64()))
        .collect();
    got.sort();

    let mut want: Vec<(String, Option<u64>)> = expected
        .iter()
        .map(|(code, line)| (code.to_string(), Some(*line)))
        .collect();
    want.sort();

    assert_eq!(got, want, "fixture {fixture}");
}

#[test]
fn cli_package_exports_conditional_types_basic_resolves_types_condition() {
    assert_fixture_codes_at_lines("package-exports-conditional-types-basic", &[("TS2322", 4)]);
}

#[test]
fn cli_package_exports_subpath_basic_resolves_subpath_types() {
    assert_fixture_codes_at_lines("package-exports-subpath-basic", &[("TS2322", 4)]);
}

#[test]
fn cli_package_exports_pattern_basic_resolves_wildcard_subpath() {
    assert_fixture_codes_at_lines("package-exports-pattern-basic", &[("TS2322", 4)]);
}

#[test]
fn cli_package_exports_custom_condition_basic_selects_development_branch() {
    assert_fixture_codes_at_lines("package-exports-custom-condition-basic", &[("TS2322", 4)]);
}

#[test]
fn cli_package_typesversions_basic_rewrites_root_types() {
    assert_fixture_codes_at_lines("package-typesversions-basic", &[("TS2322", 4)]);
}

#[test]
fn cli_package_typesversions_subpath_basic_rewrites_exact_and_pattern() {
    assert_fixture_codes_at_lines(
        "package-typesversions-subpath-basic",
        &[("TS2322", 5), ("TS2322", 7)],
    );
}

#[test]
fn cli_package_imports_field_basic_resolves_internal_alias() {
    assert_fixture_codes_at_lines("package-imports-field-basic", &[("TS2322", 4)]);
}

#[test]
fn cli_package_imports_pattern_basic_resolves_alias_wildcard() {
    assert_fixture_codes_at_lines("package-imports-pattern-basic", &[("TS2322", 4)]);
}

#[test]
fn cli_package_self_name_import_basic_resolves_own_exports() {
    assert_fixture_codes_at_lines(
        "package-self-name-import-basic",
        &[("TS2322", 5), ("TS2322", 7)],
    );
}

#[test]
fn cli_package_exports_unresolved_no_cascade_reports_ts2307_only() {
    assert_fixture_codes_at_lines("package-exports-unresolved-no-cascade", &[("TS2307", 1)]);
}

#[test]
fn cli_package_exports_missing_export_basic_reports_ts2305() {
    assert_fixture_codes_at_lines("package-exports-missing-export-basic", &[("TS2305", 1)]);
}

#[test]
fn cli_package_exports_stub_external_preserved_basic_default_reports_ts2307_and_ts2322() {
    assert_fixture_codes_at_lines(
        "package-exports-stub-external-preserved-basic",
        &[("TS2307", 1), ("TS2322", 4)],
    );
}

#[test]
fn cli_package_exports_stub_external_preserved_basic_stub_suppresses_only_unresolved() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/package-exports-stub-external-preserved-basic/tsconfig.json",
        "--stubExternalModules",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    // Unresolved external package is suppressed under stub mode...
    assert!(!codes.contains(&"TS2307".to_string()), "{codes:?}");
    // ...but errors inside the resolved package declaration are preserved.
    assert!(codes.contains(&"TS2322".to_string()), "{codes:?}");
}

#[test]
fn cli_no_implicit_any_uninitialized_let_basic_matches_typescript() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/no-implicit-any-uninitialized-let-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(!codes.contains(&"TS7005".to_string()));
}

#[test]
fn cli_jwt_payload_same_file_visibility_basic_reports_type_error_only() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/jwt-payload-same-file-visibility-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2322".to_string()));
    assert!(!codes.contains(&"TS2304".to_string()));
}

#[test]
fn cli_imported_interface_extends_downstream_assignability_basic_reports_missing_property() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/imported-interface-extends-downstream-assignability-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2741".to_string()) || codes.contains(&"TS2322".to_string()));
    assert!(!codes.contains(&"TS2304".to_string()));
}

#[test]
fn cli_imported_type_bindings_in_declaration_bodies_basic_resolves_imports() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/imported-type-bindings-in-declaration-bodies-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2741".to_string()) || codes.contains(&"TS2322".to_string()));
    assert!(!codes.contains(&"TS2304".to_string()));
}

#[test]
fn cli_contextual_async_object_property_return_basic_reports_type_errors_only() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/contextual-async-object-property-return-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2322".to_string()));
    assert!(!codes.contains(&"TS2304".to_string()));
}

#[test]
fn cli_async_void_promise_return_basic_exempts_awaited_void_from_ts2355() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/async-void-promise-return-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    // The only diagnostic is the intentional TS2355 on the value-promise
    // function; every `Promise<void|undefined|any>` shape (function, arrow,
    // object method, class method, alias) must stay clean.
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2355".to_string()], "got {codes:?}");
    assert_eq!(json_diagnostic_lines(&parsed, "TS2355"), vec![Some(42)]);
}

#[test]
fn cli_array_find_contextual_callback_basic_resolves_find_and_reports_property_error() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/array-find-contextual-callback-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(codes.contains(&"TS2339".to_string()));
    assert!(!codes.contains(&"TS7006".to_string()));
}

#[test]
fn cli_symbol_in_call_signature_stays_callable() {
    // A call signature that mentions `symbol` (param or return) must not poison
    // the interface's callability. Regression for `Symbol('x')` reported as
    // TS2349 "This expression is not callable".
    let root = temp_dir("symbol-callable");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        interface MySymbolCtor {
          (description?: string | number): symbol;
          for(key: string): symbol;
        }
        declare const make: MySymbolCtor;
        export const made = make('x');
        export const bridged: symbol = make(1);
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(
        !codes.contains(&"TS2349".to_string()),
        "symbol-typed call signature should remain callable, got {codes:?}"
    );
}

#[test]
fn cli_definite_assignment_exempts_any_and_undefined_typed_let() {
    // tsc skips definite-assignment analysis for a binding whose declared type
    // permits `undefined` (`any`, or a union containing `undefined`). Regression
    // for TS2454 on `let x: any` / `let x: T | undefined` assigned in try/loop.
    let root = temp_dir("ts2454-any-undef");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        export function anyVar() {
          let x: any;
          for (const i of [1, 2]) { x = i; }
          return x === undefined ? 0 : x + 1;
        }
        export function undefUnion() {
          let u: number | undefined;
          try { u = 1; } catch {}
          return u ? u : 0;
        }
        export function stillReports() {
          let s: string;
          return s.length;
        }
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    // The `any` and `T | undefined` bindings are exempt; the `string` binding is
    // genuinely used before assignment and must still report exactly one TS2454.
    let ts2454_count = json_diagnostic_codes(&parsed)
        .iter()
        .filter(|c| *c == "TS2454")
        .count();
    assert_eq!(
        ts2454_count, 1,
        "only the `string` binding should report TS2454"
    );
}

#[test]
fn cli_function_body_resolves_later_module_const() {
    // A function body may reference a module-scope `const` declared after it; the
    // body runs once the module is fully evaluated. Regression for TS2304 on
    // forward references like ky's `deepMerge`.
    let root = temp_dir("forward-const-ref");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        export const useEarly = () => later(1);
        const later = (x: number) => x + 1;
        export function stillReports() { return totallyMissing(1); }
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    // The forward reference to `later` resolves (no TS2304 for it); a genuinely
    // undefined name still reports exactly one TS2304.
    let ts2304_count = codes.iter().filter(|c| *c == "TS2304").count();
    assert_eq!(
        ts2304_count, 1,
        "only the genuinely-missing name should report TS2304, got {codes:?}"
    );
}

#[test]
fn cli_typeof_value_in_type_query_resolves_via_module_table() {
    // A `typeof X` type query resolving a value during statement checking must
    // consult the module's full value table, not just the active type-resolution
    // scope. Regression for ky's `readonly retry: typeof retry` (TS2304) and zod's
    // `(typeof ZodString)["create"]`: the value binding isn't in the active scope
    // when the query is resolved, but it is a module value.
    let root = temp_dir("typeof-value-query");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        export class Foo {
          static create(x: number) { return new Foo(); }
        }
        export const coerce = {
          make: ((a) => Foo.create(a)) as (typeof Foo)["create"],
        };
        export const missing = (() => 0) as typeof totallyUndefined;
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    // `typeof Foo` resolves (no TS2304 for it); only the genuinely-undefined
    // `typeof totallyUndefined` reports, and exactly once.
    let ts2304_count = codes.iter().filter(|c| *c == "TS2304").count();
    assert_eq!(
        ts2304_count, 1,
        "only the genuinely-missing typeof target should report TS2304, got {codes:?}"
    );
}

#[test]
fn cli_any_typed_callee_is_callable() {
    // Calling an `any`-typed value is allowed and yields `any` — it must not
    // report TS2349. Regression for ky's `for (const hook of hooks?.init ?? [])
    // hook(opts)`, where `?? []` widens the iterated element to `any`.
    let root = temp_dir("any-callee");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        type Hook = (x: number) => void;
        declare const hooks: { init?: Hook[] };
        export function run() {
          for (const hook of hooks.init ?? []) {
            hook(1);
          }
        }
        declare const f: any;
        export const r = f(1, 2, 3);
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(
        !codes.contains(&"TS2349".to_string()),
        "calling an any-typed value must not report TS2349, got {codes:?}"
    );
}

#[test]
fn cli_and_chain_narrows_truthy_property() {
    // `a.b && a.b > c` evaluates the right side only when `a.b` is truthy, so
    // `a.b` narrows to non-nullish there — no TS2365 on `number | undefined`.
    let root = temp_dir("and-truthy-narrow");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "strict": true }, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        declare const c: { x?: number };
        declare const p: { i: number };
        export const r = c.x && p.i > c.x;
        export function f() {
          if (c.x && p.i > c.x) { return 1; }
          return 0;
        }
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(
        !codes.contains(&"TS2365".to_string()),
        "`a.b &&` must narrow `a.b` to non-nullish in the right operand, got {codes:?}"
    );
}

#[test]
fn cli_or_of_instanceof_guards_narrows_union() {
    // `if (x instanceof A || x instanceof B)` narrows `x` to `A | B` in the
    // then-branch (dropping other members). Regression for ky's
    // `body instanceof ArrayBuffer || ArrayBuffer.isView(body)` (TS2339 on a
    // non-narrowed `ReadableStream` member).
    let root = temp_dir("or-instanceof-narrow");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        class A { byteLength = 2; }
        class B { byteLength = 3; }
        type U = A | B | string;
        export const orNarrow = (x: U): number => {
          if (x instanceof A || x instanceof B) {
            return x.byteLength;
          }
          return 0;
        };
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(
        !codes.contains(&"TS2339".to_string()),
        "an `||` of instanceof guards must narrow the union, got {codes:?}"
    );
}

#[test]
fn cli_computed_key_index_on_object_is_not_missing_property() {
    // `obj[k]` where `k` is a non-literal computed key (`keyof T` or a type
    // parameter `K extends keyof T`) resolves to an indexed-access type — it is
    // not a missing-property error. Regression for TS2339 that mis-named the
    // receiver identifier as the absent property (ky's `incoming[property]`).
    let root = temp_dir("computed-key-index");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        type Hooks = { init?: number[]; before?: string[] };
        declare const h: Hooks;
        declare const k: keyof Hooks;
        export const viaKeyof = h[k];
        export function viaGeneric<K extends keyof Hooks>(o: Hooks, p: K) {
          return o[p];
        }
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(
        !codes.contains(&"TS2339".to_string()),
        "computed-key index access must not report TS2339, got {codes:?}"
    );
}

#[test]
fn cli_typeof_guard_narrows_union_in_all_branches() {
    // `typeof x === "tag"` must narrow the union in the then-branch, the else
    // branch, AND the fall-through after an early-returning `if`. Regression for
    // ky's `if (typeof retry === 'number') return …; retry.methods`.
    let root = temp_dir("typeof-narrowing");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        export function earlyReturn(x: number | string) {
          if (typeof x === 'number') { return 0; }
          return x.length;
        }
        export function withElse(x: number | { a: number }) {
          if (typeof x === 'number') { return x; }
          else { return x.a; }
        }
        export function discriminantElse(
          v: { kind: 'a'; x: number } | { kind: 'b'; y: string },
        ) {
          if (v.kind === 'a') { return v.x; }
          return v.y;
        }
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(
        !codes.contains(&"TS2339".to_string()),
        "typeof/discriminant guards must narrow in every branch, got {codes:?}"
    );
}

#[test]
fn cli_instanceof_guard_narrows_union() {
    // `x instanceof Ctor` narrows a union nominally in then/else/early-return.
    let root = temp_dir("instanceof-narrowing");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        class Cat { meow(): number { return 1; } }
        class Dog { bark(): number { return 2; } }
        export function sound(animal: Cat | Dog) {
          if (animal instanceof Cat) { return animal.meow(); }
          return animal.bark();
        }
        export function withElse(animal: Cat | Dog) {
          if (animal instanceof Dog) { return animal.bark(); }
          else { return animal.meow(); }
        }
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert!(
        !json_diagnostic_codes(&parsed).contains(&"TS2339".to_string()),
        "instanceof guards must narrow the union in every branch"
    );
}

#[test]
fn cli_tuple_exposes_array_methods() {
    // A tuple (e.g. `[...] as const`) is an array and carries array methods.
    let root = temp_dir("tuple-array-methods");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        const methods = ['get', 'post', 'put'] as const;
        export const hasGet = methods.includes('get');
        export const upper = methods.map(m => m.toUpperCase());
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    assert!(
        !json_diagnostic_codes(&parsed).contains(&"TS2339".to_string()),
        "tuple array methods must resolve"
    );
}

#[test]
fn cli_string_keyed_mapped_type_resolves_to_index_signature() {
    // `{ [P in string]: T }` (and `Record<string, T>` routed through its mapped
    // body) must resolve to a string index signature, not collapse to `unknown`.
    // Regression for ky's `SearchParamsOption` union rendering a `Record` member
    // as `unknown`. Probe: the index value type must be `number`, so assigning it
    // to `string` is a genuine TS2322 — which only appears if it did NOT collapse
    // to `unknown` (surge treats `unknown` leniently and would emit nothing).
    let root = temp_dir("mapped-index-sig");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": {}, "include": ["src/**/*.ts"] }"#,
    );
    write_file(
        &root,
        "src/index.ts",
        r#"
        type Idx = { [P in string]: number };
        const d = {} as Idx;
        export const mismatch: string = d.anyKey;
        "#,
    );

    let project = root.join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(
        codes.contains(&"TS2322".to_string()),
        "index value type must resolve to `number` (not `unknown`), got {codes:?}"
    );
}

#[test]
fn cli_random_return_flow_authkit_shape_does_not_emit_ts2366() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/random-return-flow-authkit-shape/tsconfig.json",
        "--format",
        "json",
    ]);

    let codes = json_diagnostic_codes(&parsed);
    assert!(!codes.contains(&"TS2366".to_string()));
}

#[test]
fn cli_skip_lib_check_dependency_dts_loads_as_symbol_source_without_noise() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/skip-lib-check-dependency-dts/tsconfig.json",
        "--compatReport",
        "--format",
        "json",
    ]);

    assert_eq!(parsed["loadedDependencyDeclarationFiles"], Value::from(1));
    assert_eq!(
        parsed["diagnosticsDependencyDeclarationTotal"],
        Value::from(0)
    );
    assert_eq!(parsed["diagnosticsTotal"], Value::from(0));
}

#[test]
fn cli_configured_types_node_loads_at_types_declarations() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/configured-types-node-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    // `process` resolves through configured @types/node; only the `bad`
    // assignment mismatch should remain (no unresolved `process`/`NodeJS`).
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2322".to_string()]);
    assert!(!codes.contains(&"TS2304".to_string()));
    assert!(!codes.contains(&"TS2591".to_string()));
}

#[test]
fn cli_configured_types_scoped_maps_to_at_types_scope_dir() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/configured-types-scoped-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    // `@scope/pkg` maps to node_modules/@types/scope__pkg, whose `declare const`
    // becomes a global. Only the `bad` assignment mismatch should remain.
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2322".to_string()]);
    assert!(!codes.contains(&"TS2304".to_string()));
}

#[test]
fn cli_configured_types_missing_reports_ts2688() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/configured-types-missing-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    let diagnostics = json_diagnostics(&parsed);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], Value::from("TS2688"));
    assert_eq!(diagnostics[0]["fileName"], Value::from(""));
    assert_eq!(
        diagnostics[0]["message"],
        Value::from("Cannot find type definition file for 'configured-types-missing-pkg'.")
    );
}

#[test]
fn cli_configured_types_no_node_does_not_autoload_at_types() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/configured-types-no-node-basic/tsconfig.json",
        "--format",
        "json",
    ]);

    // Without `compilerOptions.types`, configured @types are not pulled in, so
    // `process` stays unresolved and surfaces the install-@types/node hint.
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2591".to_string()]);
}

#[test]
fn cli_auto_types_wildcard_discovers_node() {
    // `types: ["*"]` auto-discovers the visible @types/node package; `process`
    // resolves and only the `bad` assignment mismatch remains.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/auto-types-node-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2322".to_string()]);
}

#[test]
fn cli_auto_types_empty_types_disables_discovery() {
    // `types: []` disables automatic inclusion, so `process` is unresolved.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/auto-types-disabled-empty-types-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2591".to_string()]);
}

#[test]
fn cli_auto_types_narrowed_loads_only_listed() {
    // `types: ["node"]` loads node but not the visible react package.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/auto-types-narrowed-types-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2304".to_string()]);
}

#[test]
fn cli_auto_types_wildcard_discovers_scoped_package() {
    // `types: ["*"]` discovers @types/scope__pkg and unmangles it to a global.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/auto-types-scoped-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2322".to_string()]);
}

#[test]
fn cli_auto_types_wildcard_discovers_ancestor_node() {
    // The visible @types/node lives in an ancestor directory; `types: ["*"]`
    // walks up the node_modules/@types chain to find it.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/auto-types-ancestor-visibility-basic/packages/app/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2322".to_string()]);
}

#[test]
fn cli_auto_types_nearest_package_wins() {
    // The app-local @types/node (`marker.token: string`) wins over the ancestor
    // copy (`marker.token: number`); only the `number` mismatch remains.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/auto-types-nearest-wins-basic/packages/app/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2322".to_string()]);
    let lines = json_diagnostic_lines(&parsed, "TS2322");
    assert_eq!(lines, vec![Some(2)]);
}

#[test]
fn cli_type_roots_wildcard_includes_custom_root_package() {
    // `typeRoots: ["./custom-types"]` with `types: ["*"]` discovers the package
    // under the custom root.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/type-roots-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2322".to_string()]);
}

#[test]
fn cli_type_roots_ignores_default_node_modules() {
    // With `typeRoots` set, the default node_modules/@types/node is NOT consulted,
    // so `process` stays unresolved and surfaces the wildcard install hint (TS2580).
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/type-roots-ignore-default-node-modules-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2580".to_string()]);
}

#[test]
fn cli_type_roots_with_types_filter_loads_only_listed() {
    // `typeRoots` + `types: ["node"]` loads only node from the custom root; react
    // under the same root is excluded.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/type-roots-with-types-filter-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let codes = json_diagnostic_codes(&parsed);
    assert_eq!(codes, vec!["TS2304".to_string()]);
}

#[test]
fn cli_reference_types_directive_loads_node_at_types() {
    // `/// <reference types="node" />` resolves @types/node even though
    // `compilerOptions.types` is empty; `process` resolves and there are no errors.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-node-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_codes(&parsed), Vec::<String>::new());
}

#[test]
fn cli_reference_types_directive_resolves_scoped_package() {
    // `types="@scope/pkg"` maps to node_modules/@types/scope__pkg and contributes
    // its `declare const` as a global.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-scoped-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_codes(&parsed), Vec::<String>::new());
}

#[test]
fn cli_reference_types_directive_follows_recursive_references() {
    // pkg-a's own `/// <reference types="pkg-b" />` is followed, so pkg-b's
    // interface is visible to the root file.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-recursive-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_codes(&parsed), Vec::<String>::new());
}

#[test]
fn cli_reference_types_directive_missing_reports_located_ts2688() {
    // A missing reference in a root source file reports TS2688 located at the
    // specifier, matching tsc.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-missing-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    let diagnostics = json_diagnostics(&parsed);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["code"], Value::from("TS2688"));
    assert_eq!(diagnostics[0]["fileName"], Value::from("src/index.ts"));
    assert_eq!(diagnostics[0]["line"], Value::from(1));
    assert_eq!(diagnostics[0]["column"], Value::from(23));
    assert_eq!(
        diagnostics[0]["message"],
        Value::from("Cannot find type definition file for 'reference-types-missing-pkg'.")
    );
}

#[test]
fn cli_reference_types_directive_in_dependency_dts_is_followed() {
    // A reference directive inside a resolved package's declaration file pulls in
    // its @types dependency, so the dependency's type resolves with no errors.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-dependency-dts-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_codes(&parsed), Vec::<String>::new());
}

#[test]
fn cli_reference_types_missing_in_dependency_dts_respects_skip_lib_check() {
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-missing-dependency-dts-skip-lib-check-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_codes(&parsed), Vec::<String>::new());
}

#[test]
fn cli_reference_types_directive_resolves_through_type_roots() {
    // With custom `typeRoots`, the directive resolves only through the allowed
    // root (verbatim name, no @types mangling).
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-with-type-roots-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_codes(&parsed), Vec::<String>::new());
}

#[test]
fn cli_reference_types_directive_dedupes_across_files() {
    // The same referenced package appears in two files; it loads once and the
    // shared global resolves in both with no errors.
    let parsed = run_cli_json(&[
        "--project",
        "../../tests/compat-projects/reference-types-dedupe-order-basic/tsconfig.json",
        "--format",
        "json",
    ]);
    assert_eq!(json_diagnostic_codes(&parsed), Vec::<String>::new());
}

fn run_compat_fixture_codes(fixture: &str) -> Vec<String> {
    let project = compat_project_root(fixture).join("tsconfig.json");
    let project = project.to_string_lossy().into_owned();
    let parsed = run_cli_json(&["--project", project.as_str(), "--format", "json"]);
    json_diagnostic_codes(&parsed)
}

#[test]
fn project_mode_interface_merging_same_scope() {
    assert_eq!(
        run_compat_fixture_codes("interface-merging-basic"),
        vec!["TS2741", "TS2322"]
    );
}

#[test]
fn project_mode_interface_merging_across_files() {
    assert_eq!(
        run_compat_fixture_codes("interface-merging-across-files-basic"),
        vec!["TS2741", "TS2322"]
    );
}

#[test]
fn project_mode_interface_merging_property_conflict() {
    // Incompatible property types across merged interfaces report TS2717 once;
    // the first declaration's type wins, so the assignment does not cascade.
    assert_eq!(
        run_compat_fixture_codes("interface-merging-conflict-basic"),
        vec!["TS2717"]
    );
}

#[test]
fn project_mode_declare_global_interface_merging() {
    assert_eq!(
        run_compat_fixture_codes("declare-global-interface-basic"),
        vec!["TS2741", "TS2322"]
    );
}

#[test]
fn project_mode_declare_global_window_physical_lib() {
    if !typescript_lib_available() {
        eprintln!("skipping: node_modules/typescript not installed");
        return;
    }
    assert_eq!(
        run_physical_fixture_codes("declare-global-window-physical-lib-basic"),
        vec!["TS2322"]
    );
}

#[test]
fn project_mode_module_augmentation_package_interface() {
    assert_eq!(
        run_compat_fixture_codes("module-augmentation-package-interface-basic"),
        vec!["TS2741", "TS2322"]
    );
}

#[test]
fn project_mode_module_augmentation_add_export() {
    assert_eq!(
        run_compat_fixture_codes("module-augmentation-add-export-basic"),
        vec!["TS2322", "TS2345"]
    );
}

#[test]
fn project_mode_ambient_module_reopen_merge() {
    assert_eq!(
        run_compat_fixture_codes("ambient-module-reopen-merge-basic"),
        vec!["TS2741"]
    );
}

#[test]
fn project_mode_module_augmentation_unresolved_no_cascade() {
    // Augmenting an unresolved module is reported once as TS2307 with no
    // downstream cascade from the unresolved import binding.
    assert_eq!(
        run_compat_fixture_codes("module-augmentation-unresolved-no-cascade"),
        vec!["TS2307"]
    );
}

#[test]
fn project_mode_interface_method_merge() {
    assert_eq!(
        run_compat_fixture_codes("interface-method-merge-basic"),
        vec!["TS2322", "TS2345"]
    );
}

#[test]
fn project_mode_class_interface_merge_instance_members() {
    // Class/interface merging contributes the interface's instance members to
    // the class type, matching tsc (no diagnostics for this fixture).
    assert!(run_compat_fixture_codes("class-interface-merge-policy-pinned").is_empty());
}
