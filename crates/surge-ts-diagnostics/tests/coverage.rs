use std::{collections::HashMap, fs, path::PathBuf};

use serde::Deserialize;
use surge_ts_diagnostics_codegen::{DiagnosticSupport, load_catalog};

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    status: String,
    callsite_groups: Option<Vec<String>>,
    fixtures: Option<Vec<String>>,
    span_tests: Option<Vec<String>>,
    oracle: Option<Vec<String>>,
    reason: Option<String>,
}

type Manifest = HashMap<String, ManifestEntry>;

fn catalog_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diagnostic-messages.json")
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/emitted-diagnostics.toml")
}

fn load_manifest() -> Manifest {
    toml::from_str(&fs::read_to_string(manifest_path()).expect("manifest readable"))
        .expect("manifest parses")
}

#[test]
fn emitted_diagnostics_have_manifest_entry() {
    let catalog = load_catalog(catalog_path()).expect("catalog loads");
    let manifest = load_manifest();

    for entry in catalog {
        if matches!(entry.support, DiagnosticSupport::Emitted) {
            let m = manifest.get(&entry.code).unwrap_or_else(|| {
                panic!("Emitted diagnostic {} is missing from manifest", entry.code)
            });
            assert_eq!(
                m.status, "emitted",
                "Manifest status for {} must be 'emitted'",
                entry.code
            );

            let fixtures = m.fixtures.as_deref().unwrap_or(&[]);
            let span_tests = m.span_tests.as_deref().unwrap_or(&[]);
            let oracle = m.oracle.as_deref().unwrap_or(&[]);

            assert!(
                !fixtures.is_empty() || !span_tests.is_empty() || !oracle.is_empty(),
                "Emitted diagnostic {} must have at least one fixture, span_test, or oracle evidence",
                entry.code
            );
        }
    }
}

#[test]
fn catalog_only_have_reason_if_callsites_exist() {
    let catalog = load_catalog(catalog_path()).expect("catalog loads");
    let manifest = load_manifest();

    for entry in catalog {
        if matches!(entry.support, DiagnosticSupport::CatalogOnly) {
            let m = manifest.get(&entry.code).unwrap_or_else(|| {
                panic!(
                    "Catalog-only diagnostic {} is missing from manifest",
                    entry.code
                )
            });
            assert_eq!(
                m.status, "catalog-only",
                "Manifest status for {} must be 'catalog-only'",
                entry.code
            );

            if m.callsite_groups.as_deref().is_some_and(|g| !g.is_empty()) {
                let reason = m.reason.as_deref().unwrap_or("");
                assert!(
                    !reason.is_empty(),
                    "Catalog-only diagnostic {} with callsites must have a reason",
                    entry.code
                );
            }
        }
    }
}

#[test]
fn specific_diagnostics_classified_correctly() {
    let catalog = load_catalog(catalog_path()).expect("catalog loads");

    for code in ["TS2307", "TS2314", "TS2315"] {
        let entry = catalog
            .iter()
            .find(|e| e.code == code)
            .expect("Diagnostic should exist");
        assert!(
            matches!(entry.support, DiagnosticSupport::Emitted),
            "{} should be classified as emitted",
            code
        );
    }
}
