use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use surge_ts_checker::{
    CheckerOptions, DiagnosticProfile, SourceFileInput, check_program, check_program_with_options,
    check_source, check_source_with_options,
};
use surge_ts_diagnostics::render_diagnostics;

struct VirtualFile {
    file_name: String,
    source_text: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    case: Vec<ManifestCase>,
}

#[derive(Debug, Deserialize)]
struct ManifestCase {
    name: String,
    status: String,
    upstream_repo: String,
    upstream_commit: String,
    upstream_path: String,
    upstream_baseline_path: String,
    local_path: String,
    mode: String,
    #[serde(default)]
    expected_diagnostics: Vec<String>,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct SmokeManifest {
    case: Vec<SmokeCase>,
}

#[derive(Debug, Deserialize)]
struct SmokeFile {
    file_name: String,
    source_text: String,
}

#[derive(Debug, Deserialize)]
struct SmokeCase {
    name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    files: Vec<SmokeFile>,
    #[serde(default)]
    expected_diagnostics: Vec<String>,
    #[serde(default)]
    no_implicit_any: bool,
}

#[test]
fn smoke_cases_emit_expected_codes() {
    let manifest = load_smoke_manifest();

    for case in manifest.case {
        let use_native_profile = case
            .expected_diagnostics
            .iter()
            .any(|code| code.starts_with("surge::"));

        let (diagnostics, rendered) = if !case.files.is_empty() {
            let inputs = case
                .files
                .iter()
                .map(|file| SourceFileInput {
                    file_name: file.file_name.clone(),
                    source_text: file.source_text.clone(),
                })
                .collect::<Vec<_>>();
            let diagnostics = if case.no_implicit_any || use_native_profile {
                check_program_with_options(
                    inputs,
                    smoke_checker_options(case.no_implicit_any, use_native_profile),
                )
            } else {
                check_program(inputs)
            };

            let rendered = render_program_diagnostics(&case.files, &diagnostics);
            (diagnostics, rendered)
        } else {
            let path = workspace_root().join(case.path.as_ref().unwrap_or_else(|| {
                panic!("smoke case {} is missing both path and files", case.name)
            }));
            let source = fs::read_to_string(&path).unwrap_or_else(|error| {
                panic!(
                    "failed to read smoke case {} at {}: {error}",
                    case.name,
                    path.display()
                );
            });
            let diagnostics = if case.no_implicit_any || use_native_profile {
                check_source_with_options(
                    &source,
                    path.to_string_lossy().as_ref(),
                    smoke_checker_options(case.no_implicit_any, use_native_profile),
                )
            } else {
                check_source(&source, path.to_string_lossy().as_ref())
            };
            let rendered = render_diagnostics(&diagnostics, &source);
            (diagnostics, rendered)
        };
        let actual_codes = diagnostic_codes(&diagnostics);

        assert_eq!(
            actual_codes,
            case.expected_diagnostics,
            "unexpected diagnostics for smoke case {} at {}\nrendered diagnostics:\n{}",
            case.name,
            case.path.as_deref().unwrap_or("<program files>"),
            rendered
        );
    }
}

fn smoke_checker_options(no_implicit_any: bool, use_native_profile: bool) -> CheckerOptions {
    CheckerOptions {
        resolved_modules: Default::default(),
        stub_external_modules: false,
        no_implicit_any,
        no_lib: false,
        diagnostic_profile: if use_native_profile {
            DiagnosticProfile::Native
        } else {
            DiagnosticProfile::Tsc
        },
        ..Default::default()
    }
}

#[test]
fn active_upstream_cases_emit_expected_codes() {
    let manifest = load_manifest();

    for case in manifest
        .case
        .into_iter()
        .filter(|case| case.status == "active")
    {
        let path = workspace_root().join(&case.local_path);
        let source = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read active upstream case {} at {}: {error}",
                case.name,
                path.display()
            );
        });

        let virtual_files = match case.mode.as_str() {
            "single_source" => vec![VirtualFile {
                file_name: case.local_path.clone(),
                source_text: source.clone(),
            }],
            "virtual_files" => split_typescript_testdata_virtual_files(&source, &case.local_path),
            other => panic!(
                "unknown upstream fixture mode {other:?} for active upstream case {}",
                case.name
            ),
        };

        let diagnostics = if case.mode == "virtual_files" {
            let inputs = virtual_files
                .iter()
                .map(|virtual_file| SourceFileInput {
                    file_name: virtual_file.file_name.clone(),
                    source_text: virtual_file.source_text.clone(),
                })
                .collect();

            check_program(inputs)
        } else {
            let mut diagnostics = Vec::new();

            for virtual_file in &virtual_files {
                diagnostics.extend(check_source(
                    &virtual_file.source_text,
                    &virtual_file.file_name,
                ));
            }

            diagnostics
        };

        let rendered_diagnostics = render_virtual_file_diagnostics(&virtual_files, &diagnostics);

        let actual_codes = diagnostic_codes(&diagnostics);

        assert_eq!(
            actual_codes,
            case.expected_diagnostics,
            "unexpected diagnostics for active upstream case {} ({}) from {}@{} at {} (baseline {}, mode {})\nrendered diagnostics:\n{}",
            case.name,
            case.reason,
            case.upstream_repo,
            case.upstream_commit,
            case.upstream_path,
            case.upstream_baseline_path,
            case.mode,
            rendered_diagnostics
        );
    }
}

#[test]
fn manifest_contains_at_least_one_active_case() {
    let manifest = load_manifest();
    assert!(
        manifest.case.iter().any(|case| case.status == "active"),
        "expected at least one active upstream case"
    );
}

#[test]
fn object_literal_excess_property_uses_first_source_order_property() {
    let source = "let user: { name: string } = { name: \"Ada\", age: 36, active: true };";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2353"]);
    assert!(
        diagnostics[0].message.contains("age"),
        "unexpected TS2353 message: {rendered}"
    );
}

#[test]
fn named_interface_shown_by_name_in_excess_property() {
    let source = "interface I { a: number } let s: I = { a: 1, b: 2 };";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2353"]);
    assert!(
        diagnostics[0]
            .message
            .contains("does not exist in type 'I'"),
        "expected interface name in TS2353 message: {rendered}"
    );
}

#[test]
fn named_type_alias_shown_by_name_in_excess_property() {
    let source = "type T = { a: number }; let s: T = { a: 1, b: 2 };";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2353"]);
    assert!(
        diagnostics[0]
            .message
            .contains("does not exist in type 'T'"),
        "expected type-alias name in TS2353 message: {rendered}"
    );
}

#[test]
fn named_interface_shown_by_name_in_missing_property() {
    let source = "interface I { a: number; z: number } let s: I = { a: 1 };";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2741"]);
    assert!(
        diagnostics[0].message.contains("required in type 'I'"),
        "expected interface name as TS2741 target: {rendered}"
    );
}

#[test]
fn argument_literal_widens_to_base_for_non_literal_target() {
    let source = "declare function g(x: string): void; g(1);";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2345"]);
    assert!(
        diagnostics[0].message.contains("Argument of type 'number'"),
        "expected widened source type in TS2345 message: {rendered}"
    );
}

#[test]
fn argument_literal_kept_for_literal_target() {
    let source = "declare function f(x: \"a\"): void; f(\"b\");";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2345"]);
    assert!(
        diagnostics[0].message.contains("Argument of type '\"b\"'"),
        "expected literal source kept against literal target in TS2345 message: {rendered}"
    );
}

#[test]
fn assignment_literal_kept_for_literal_target() {
    let source = "let y: 2 = 1;";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2322"]);
    assert!(
        diagnostics[0]
            .message
            .contains("Type '1' is not assignable to type '2'"),
        "expected literal source kept against literal target in TS2322 message: {rendered}"
    );
}

#[test]
fn equality_operands_widen_for_display() {
    let source = "let r = (1 === \"string\");";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2367"]);
    assert!(
        diagnostics[0]
            .message
            .contains("types 'number' and 'string'"),
        "expected widened operands in TS2367 message: {rendered}"
    );
}

#[test]
fn object_literal_missing_property_uses_first_target_order_property() {
    let source = "let user: { name: string; alpha: number } = {};";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2741"]);
    assert!(
        diagnostics[0].message.contains("alpha"),
        "unexpected TS2741 message: {rendered}"
    );
}

#[test]
fn object_literal_missing_property_does_not_cascade_from_unresolved_value() {
    let source = "let user: { name: string; age: number } = { name: missing };";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2304"]);
    assert!(
        diagnostics[0].message.contains("missing"),
        "unexpected TS2304 message: {rendered}"
    );
}

#[test]
fn type_aliases_are_desugared_in_diagnostic_messages() {
    let source = "type Name = string; let value: Name = 123;";
    let diagnostics = check_source(source, "example.ts");
    let rendered = render_diagnostics(&diagnostics, source);

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2322"]);
    assert!(
        diagnostics[0].message.contains("string"),
        "expected alias target name in TS2322 message: {rendered}"
    );
    assert!(
        !diagnostics[0].message.contains("Name"),
        "expected alias name to be desugared in TS2322 message: {rendered}"
    );
}

fn load_manifest() -> Manifest {
    let manifest_path = workspace_root().join("tests/upstream/typescript-go/manifest.toml");
    let manifest_text = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", manifest_path.display());
    });

    toml::from_str(&manifest_text).unwrap_or_else(|error| {
        panic!(
            "failed to parse {} as TOML: {error}",
            manifest_path.display()
        );
    })
}

fn load_smoke_manifest() -> SmokeManifest {
    let manifest_path = workspace_root().join("tests/smoke/manifest.toml");
    let manifest_text = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", manifest_path.display());
    });

    toml::from_str(&manifest_text).unwrap_or_else(|error| {
        panic!(
            "failed to parse {} as TOML: {error}",
            manifest_path.display()
        );
    })
}

fn diagnostic_codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn render_program_diagnostics(
    files: &[SmokeFile],
    diagnostics: &[surge_ts_diagnostics::Diagnostic],
) -> String {
    let mut sources_by_file = HashMap::new();
    for file in files {
        sources_by_file.insert(file.file_name.clone(), file.source_text.clone());
    }

    let mut diagnostics_by_file: HashMap<String, Vec<surge_ts_diagnostics::Diagnostic>> =
        HashMap::new();
    for diagnostic in diagnostics {
        diagnostics_by_file
            .entry(diagnostic.file_name.clone())
            .or_default()
            .push(diagnostic.clone());
    }

    let mut rendered = Vec::new();
    for file in files {
        let Some(file_diagnostics) = diagnostics_by_file.remove(&file.file_name) else {
            continue;
        };

        if file_diagnostics.is_empty() {
            continue;
        }

        let Some(source_text) = sources_by_file.get(&file.file_name) else {
            continue;
        };

        rendered.push(format!(
            "virtual file: {}\n{}",
            file.file_name,
            render_diagnostics(&file_diagnostics, source_text)
        ));
    }

    if !diagnostics_by_file.is_empty() {
        let mut remaining = diagnostics_by_file.into_iter().collect::<Vec<_>>();
        remaining.sort_by(|left, right| left.0.cmp(&right.0));

        for (file_name, file_diagnostics) in remaining {
            if file_diagnostics.is_empty() {
                continue;
            }

            rendered.push(format!(
                "virtual file: {}\n{}",
                file_name,
                render_diagnostics(&file_diagnostics, "")
            ));
        }
    }

    rendered.join("\n\n")
}

fn render_virtual_file_diagnostics(
    virtual_files: &[VirtualFile],
    diagnostics: &[surge_ts_diagnostics::Diagnostic],
) -> String {
    let mut sources_by_file = HashMap::new();
    for virtual_file in virtual_files {
        sources_by_file.insert(
            virtual_file.file_name.clone(),
            virtual_file.source_text.clone(),
        );
    }

    let mut diagnostics_by_file: HashMap<String, Vec<surge_ts_diagnostics::Diagnostic>> =
        HashMap::new();
    for diagnostic in diagnostics {
        diagnostics_by_file
            .entry(diagnostic.file_name.clone())
            .or_default()
            .push(diagnostic.clone());
    }

    let mut rendered = Vec::new();
    for virtual_file in virtual_files {
        let Some(file_diagnostics) = diagnostics_by_file.remove(&virtual_file.file_name) else {
            continue;
        };

        if file_diagnostics.is_empty() {
            continue;
        }

        let Some(source_text) = sources_by_file.get(&virtual_file.file_name) else {
            continue;
        };

        rendered.push(format!(
            "virtual file: {}\n{}",
            virtual_file.file_name,
            render_diagnostics(&file_diagnostics, source_text)
        ));
    }

    if !diagnostics_by_file.is_empty() {
        let mut remaining = diagnostics_by_file.into_iter().collect::<Vec<_>>();
        remaining.sort_by(|left, right| left.0.cmp(&right.0));

        for (file_name, file_diagnostics) in remaining {
            if file_diagnostics.is_empty() {
                continue;
            }

            rendered.push(format!(
                "virtual file: {}\n{}",
                file_name,
                render_diagnostics(&file_diagnostics, "")
            ));
        }
    }

    rendered.join("\n\n")
}

fn split_typescript_testdata_virtual_files(source: &str, fallback_name: &str) -> Vec<VirtualFile> {
    const MARKER_PREFIX: &str = "// @filename:";

    let has_marker = source
        .lines()
        .any(|line| line.trim_start().starts_with(MARKER_PREFIX));
    if !has_marker {
        return vec![VirtualFile {
            file_name: fallback_name.to_string(),
            source_text: source.to_string(),
        }];
    }

    let mut virtual_files = Vec::new();
    let mut current_file_name: Option<String> = None;
    let mut current_source = String::new();
    let mut saw_marker = false;

    for line in source.split_inclusive('\n') {
        let line_without_newline = line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some(rest) = line_without_newline
            .trim_start()
            .strip_prefix(MARKER_PREFIX)
        {
            saw_marker = true;

            if let Some(file_name) = current_file_name.take() {
                virtual_files.push(VirtualFile {
                    file_name,
                    source_text: current_source,
                });
            }

            current_file_name = Some(rest.trim().to_string());
            current_source = String::new();
            continue;
        }

        if saw_marker {
            current_source.push_str(line);
        }
    }

    if let Some(file_name) = current_file_name {
        virtual_files.push(VirtualFile {
            file_name,
            source_text: current_source,
        });
    }

    virtual_files
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|error| panic!("failed to resolve workspace root: {error}"))
}

#[test]
fn program_virtual_files_cross_file_type_reference() {
    let source = r#"
// @filename: a.ts
type Name = string;
// @filename: b.ts
let value: Name = "Ada";
"#;
    let virtual_files = split_typescript_testdata_virtual_files(source, "fallback.ts");
    let diagnostics = check_program(
        virtual_files
            .iter()
            .map(|virtual_file| SourceFileInput {
                file_name: virtual_file.file_name.clone(),
                source_text: virtual_file.source_text.clone(),
            })
            .collect(),
    );

    assert!(diagnostics.is_empty());
    assert_eq!(
        virtual_files
            .iter()
            .map(|virtual_file| virtual_file.file_name.clone())
            .collect::<Vec<_>>(),
        vec!["a.ts".to_string(), "b.ts".to_string()]
    );
}

#[test]
fn program_virtual_files_diagnostics_preserve_virtual_file_names() {
    let source = r#"
// @filename: a.ts
type Name = string;
// @filename: b.ts
let value: Name = 123;
"#;
    let virtual_files = split_typescript_testdata_virtual_files(source, "fallback.ts");
    let diagnostics = check_program(
        virtual_files
            .iter()
            .map(|virtual_file| SourceFileInput {
                file_name: virtual_file.file_name.clone(),
                source_text: virtual_file.source_text.clone(),
            })
            .collect(),
    );

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(diagnostics[0].file_name, "b.ts");
}

#[test]
fn program_virtual_files_order_is_marker_order() {
    let source = r#"
// @filename: b.ts
let b: number = "x";
// @filename: a.ts
let a: number = "y";
"#;
    let virtual_files = split_typescript_testdata_virtual_files(source, "fallback.ts");
    let diagnostics = check_program(
        virtual_files
            .iter()
            .map(|virtual_file| SourceFileInput {
                file_name: virtual_file.file_name.clone(),
                source_text: virtual_file.source_text.clone(),
            })
            .collect(),
    );

    assert_eq!(diagnostic_codes(&diagnostics), vec!["TS2322", "TS2322"]);
    assert_eq!(diagnostics[0].file_name, "b.ts");
    assert_eq!(diagnostics[1].file_name, "a.ts");
}
