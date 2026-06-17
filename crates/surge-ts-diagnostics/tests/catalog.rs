use std::{collections::BTreeSet, fs, path::PathBuf};

use serde::Deserialize;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_diagnostics_codegen::{
    CatalogEntry, DiagnosticCategory as CatalogCategory, DiagnosticSource as CatalogSource,
    DiagnosticSupport as CatalogSupport, diagnostic_function_name, generate_rust,
    generate_snapshot_toml, load_catalog, validate_catalog,
};

#[derive(Debug, Deserialize)]
struct SnapshotFile {
    diagnostic: Vec<SnapshotEntry>,
}

#[derive(Debug, Deserialize)]
struct SnapshotEntry {
    code: String,
    message: String,
}

fn catalog_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("diagnostic-messages.json")
}

fn generated_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/generated.rs")
}

fn snapshot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/typescript-diagnostics/catalog.snapshot.toml")
}

fn catalog_entries() -> Vec<CatalogEntry> {
    load_catalog(catalog_path()).expect("catalog should load")
}

fn snapshot_entries() -> Vec<SnapshotEntry> {
    toml::from_str::<SnapshotFile>(&fs::read_to_string(snapshot_path()).expect("snapshot readable"))
        .expect("snapshot parses")
        .diagnostic
}

#[test]
fn catalog_json_codes_are_unique() {
    let entries = catalog_entries();
    let mut seen = BTreeSet::new();
    for entry in &entries {
        assert!(
            seen.insert(entry.code.clone()),
            "duplicate code: {}",
            entry.code
        );
    }
}

#[test]
fn catalog_json_messages_are_non_empty() {
    for entry in catalog_entries() {
        assert!(
            !entry.message.trim().is_empty(),
            "{} has an empty message",
            entry.code
        );
    }
}

#[test]
fn catalog_json_categories_are_valid() {
    validate_catalog(&catalog_entries()).expect("catalog should validate");
}

#[test]
fn catalog_json_placeholder_arity_matches() {
    validate_catalog(&catalog_entries()).expect("placeholder arity should match");
}

#[test]
fn catalog_json_custom_codes_follow_policy() {
    for entry in catalog_entries() {
        if matches!(entry.source, CatalogSource::TypescriptRust) {
            assert!(
                entry.code.starts_with("surge::"),
                "{} should be namespaced",
                entry.code
            );
            let suffix = &entry.code["surge::".len()..];
            assert!(
                suffix
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
                "{} should use kebab-case",
                entry.code
            );
        }
    }
}

#[test]
fn generated_diagnostic_function_names_are_unique() {
    let entries = catalog_entries();
    let mut seen = BTreeSet::new();
    for entry in &entries {
        let function_name = diagnostic_function_name(entry);
        assert!(
            seen.insert(function_name.clone()),
            "duplicate function name: {}",
            function_name
        );
    }
}

#[test]
fn generated_catalog_is_up_to_date() {
    let entries = catalog_entries();
    let generated = generate_rust(&entries).expect("catalog should generate");
    let committed = fs::read_to_string(generated_path()).expect("generated.rs should exist");
    assert_eq!(generated, committed);

    let snapshot = generate_snapshot_toml(&entries).expect("snapshot should generate");
    let committed_snapshot = fs::read_to_string(snapshot_path()).expect("snapshot should exist");
    assert_eq!(snapshot, committed_snapshot);
}

#[test]
fn existing_diagnostic_codes_preserved() {
    let entries = catalog_entries();
    let snapshot = snapshot_entries();

    let codes_from_catalog = entries
        .iter()
        .map(|entry| entry.code.as_str())
        .collect::<Vec<_>>();
    let codes_from_snapshot = snapshot
        .iter()
        .map(|entry| entry.code.as_str())
        .collect::<Vec<_>>();

    assert_eq!(codes_from_catalog, codes_from_snapshot);
}

#[test]
fn existing_diagnostic_codes_preserved_includes_ts2882() {
    let entries = catalog_entries();
    let snapshot = snapshot_entries();

    assert!(entries.iter().any(|entry| entry.code == "TS2882"));
    assert!(snapshot.iter().any(|entry| entry.code == "TS2882"));
}

#[test]
fn catalog_contains_ts2882() {
    let entries = catalog_entries();
    let entry = entries
        .iter()
        .find(|entry| entry.code == "TS2882")
        .expect("TS2882 should be in the catalog");

    assert_eq!(entry.category, CatalogCategory::Error);
    assert_eq!(
        entry.message,
        "Cannot find module or type declarations for side-effect import of '{0}'."
    );
    assert_eq!(entry.source, CatalogSource::Typescript);
    assert_eq!(entry.arity, 1);
    assert_eq!(entry.support, CatalogSupport::Emitted);
}

#[test]
fn existing_diagnostic_messages_preserved() {
    let entries = catalog_entries();
    let snapshot = snapshot_entries();

    let messages_from_catalog = entries
        .iter()
        .map(|entry| entry.message.as_str())
        .collect::<Vec<_>>();
    let messages_from_snapshot = snapshot
        .iter()
        .map(|entry| entry.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(messages_from_catalog, messages_from_snapshot);
}

#[test]
fn generated_ts2322_formats_message() {
    let diagnostic = Diagnostic::ts2322("string", "number", "example.ts");
    assert_eq!(diagnostic.code.to_string(), "TS2322");
    assert_eq!(
        diagnostic.message,
        "Type 'string' is not assignable to type 'number'."
    );
}

#[test]
fn generated_ts2307_formats_message() {
    let diagnostic = Diagnostic::ts2307("foo", "example.ts");
    assert_eq!(diagnostic.code.to_string(), "TS2307");
    assert_eq!(
        diagnostic.message,
        "Cannot find module 'foo' or its corresponding type declarations."
    );
}

#[test]
fn generated_ts2314_formats_message() {
    let diagnostic = Diagnostic::ts2314("Box", 2, "example.ts");
    assert_eq!(diagnostic.code.to_string(), "TS2314");
    assert_eq!(
        diagnostic.message,
        "Generic type 'Box' requires 2 type argument(s)."
    );
}

#[test]
fn generated_ts2315_formats_message() {
    let diagnostic = Diagnostic::ts2315("Name", "example.ts");
    assert_eq!(diagnostic.code.to_string(), "TS2315");
    assert_eq!(diagnostic.message, "Type 'Name' is not generic.");
}

#[test]
fn generated_ts2693_formats_message() {
    let diagnostic = Diagnostic::ts2693("Name", "example.ts");
    assert_eq!(diagnostic.code.to_string(), "TS2693");
    assert_eq!(
        diagnostic.message,
        "'Name' only refers to a type, but is being used as a value here."
    );
}

#[test]
fn generated_ts2882_formats_message() {
    let diagnostic = Diagnostic::ts2882("reflect-metadata", "example.ts");
    assert_eq!(diagnostic.code.to_string(), "TS2882");
    assert_eq!(
        diagnostic.message,
        "Cannot find module or type declarations for side-effect import of 'reflect-metadata'."
    );
}

#[test]
fn generated_custom_diagnostic_formats_message() {
    let diagnostic = Diagnostic::surge_unsupported_module_syntax("example.ts");
    assert_eq!(
        diagnostic.code.to_string(),
        "surge::unsupported-module-syntax"
    );
    assert_eq!(diagnostic.message, "Unsupported module syntax.");
}

#[test]
fn generated_wrong_arity_rejected_by_validation() {
    let invalid = vec![CatalogEntry {
        code: "TS9999".to_string(),
        category: CatalogCategory::Error,
        message: "Type '{0}' is not assignable.".to_string(),
        source: CatalogSource::Typescript,
        arity: 0,
        support: CatalogSupport::Emitted,
    }];

    assert!(validate_catalog(&invalid).is_err());
}

#[test]
fn catalog_descriptor_categories_and_supports_are_known() {
    for entry in catalog_entries() {
        match entry.category {
            CatalogCategory::Error
            | CatalogCategory::Warning
            | CatalogCategory::Suggestion
            | CatalogCategory::Message => {}
        }

        match entry.support {
            CatalogSupport::CatalogOnly | CatalogSupport::Emitted => {}
        }

        match entry.source {
            CatalogSource::Typescript | CatalogSource::TypescriptRust => {}
        }
    }
}
