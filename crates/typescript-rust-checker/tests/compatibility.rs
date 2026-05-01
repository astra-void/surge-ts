use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use typescript_rust_checker::{CheckerOptions, check_source, check_source_with_options};
use typescript_rust_diagnostics::render_diagnostics;

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
struct SmokeCase {
    name: String,
    path: String,
    #[serde(default)]
    expected_diagnostics: Vec<String>,
    #[serde(default)]
    no_implicit_any: bool,
}

#[test]
fn smoke_cases_emit_expected_codes() {
    let manifest = load_smoke_manifest();

    for case in manifest.case {
        let path = workspace_root().join(&case.path);
        let source = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read smoke case {} at {}: {error}",
                case.name,
                path.display()
            );
        });
        let diagnostics = if case.no_implicit_any {
            check_source_with_options(
                &source,
                &case.path,
                CheckerOptions {
                    no_implicit_any: true,
                },
            )
        } else {
            check_source(&source, &case.path)
        };
        let rendered = render_diagnostics(&diagnostics, &source);
        let actual_codes = diagnostic_codes(&diagnostics);

        assert_eq!(
            actual_codes, case.expected_diagnostics,
            "unexpected diagnostics for smoke case {} at {}\nrendered diagnostics:\n{}",
            case.name, case.path, rendered
        );
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

        let mut diagnostics = Vec::new();
        let mut rendered_by_virtual_file = Vec::new();

        for virtual_file in &virtual_files {
            let file_diagnostics = check_source(&virtual_file.source_text, &virtual_file.file_name);
            rendered_by_virtual_file.push(format!(
                "virtual file: {}\n{}",
                virtual_file.file_name,
                render_diagnostics(&file_diagnostics, &virtual_file.source_text)
            ));
            diagnostics.extend(file_diagnostics);
        }

        let actual_codes = diagnostic_codes(&diagnostics);
        let rendered_diagnostics = rendered_by_virtual_file.join("\n\n");

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

fn diagnostic_codes(diagnostics: &[typescript_rust_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
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
