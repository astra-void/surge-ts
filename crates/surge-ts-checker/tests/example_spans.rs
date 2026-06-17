use std::fs;
use std::path::PathBuf;

use surge_ts_checker::{CheckerOptions, check_source_with_options};
use surge_ts_diagnostics::Diagnostic;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn load_example(name: &str) -> String {
    fs::read_to_string(workspace_root().join("examples").join(name)).unwrap()
}

fn tuples(diagnostics: &[Diagnostic]) -> Vec<(String, Option<(usize, usize)>)> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code.to_string(),
                diagnostic.span.map(|span| (span.start, span.end)),
            )
        })
        .collect()
}

fn span(source: &str, needle: &str) -> (usize, usize) {
    let start = source.find(needle).unwrap();
    (start, start + needle.len())
}

fn span_nth(source: &str, needle: &str, nth: usize) -> (usize, usize) {
    let start = source
        .match_indices(needle)
        .nth(nth)
        .map(|(start, _)| start)
        .unwrap();
    (start, start + needle.len())
}

#[test]
fn examples_assignment_span_baseline() {
    let source = load_example("assignment.ts");
    let diagnostics =
        check_source_with_options(&source, "examples/assignment.ts", CheckerOptions::default());

    assert_eq!(
        tuples(&diagnostics),
        vec![("TS2322".to_string(), Some(span(&source, "1")))]
    );
}

#[test]
fn examples_function_call_span_baseline() {
    let source = load_example("function-call.ts");
    let diagnostics = check_source_with_options(
        &source,
        "examples/function-call.ts",
        CheckerOptions::default(),
    );

    assert_eq!(
        tuples(&diagnostics),
        vec![("TS2345".to_string(), Some(span(&source, "1")))]
    );
}

#[test]
fn examples_function_return_span_baseline() {
    let source = load_example("function-return.ts");
    let diagnostics = check_source_with_options(
        &source,
        "examples/function-return.ts",
        CheckerOptions::default(),
    );

    assert_eq!(
        tuples(&diagnostics),
        vec![("TS2322".to_string(), Some(span(&source, "1")))]
    );
}

#[test]
fn examples_object_property_span_baseline() {
    let source = load_example("object-property.ts");
    let diagnostics = check_source_with_options(
        &source,
        "examples/object-property.ts",
        CheckerOptions::default(),
    );

    assert_eq!(
        tuples(&diagnostics),
        vec![("TS2339".to_string(), Some(span(&source, "age")))]
    );
}

#[test]
fn examples_operator_diagnostics_span_baseline() {
    let source = load_example("operator-diagnostics.ts");
    let diagnostics = check_source_with_options(
        &source,
        "examples/operator-diagnostics.ts",
        CheckerOptions::default(),
    );

    assert_eq!(
        tuples(&diagnostics),
        vec![
            ("TS2362".to_string(), Some(span(&source, "\"x\""))),
            ("TS2365".to_string(), Some(span(&source, "true + 1"))),
            ("TS2367".to_string(), Some(span(&source, "\"x\" === 1"))),
        ]
    );
}

#[test]
fn examples_function_body_local_span_baseline() {
    let source = load_example("function-body-local.ts");
    let diagnostics = check_source_with_options(
        &source,
        "examples/function-body-local.ts",
        CheckerOptions::default(),
    );

    assert_eq!(
        tuples(&diagnostics),
        vec![("TS2322".to_string(), Some(span_nth(&source, "value", 1)))]
    );
}

#[test]
fn examples_basic_span_baseline() {
    let source = load_example("basic.ts");
    let diagnostics =
        check_source_with_options(&source, "examples/basic.ts", CheckerOptions::default());

    // `var a: string = 1;` — tsc anchors the assignability error on the
    // declaration name `a`, not the initializer.
    assert_eq!(
        tuples(&diagnostics),
        vec![("TS2322".to_string(), Some(span_nth(&source, "a", 1)))]
    );
}

#[test]
fn examples_unresolved_span_baseline() {
    let source = load_example("unresolved.ts");
    let diagnostics =
        check_source_with_options(&source, "examples/unresolved.ts", CheckerOptions::default());

    assert_eq!(
        tuples(&diagnostics),
        vec![("TS2304".to_string(), Some(span(&source, "a")))]
    );
}
