use surge_ts_checker::check_source;

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check(source_text: &str) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_source(source_text, "example.tsx")
}

const SETUP: &str = "declare namespace JSX { interface Element {} interface IntrinsicElements { div: {} } }\n\
     interface Props { task: string }\n\
     declare function Item(props: Props): JSX.Element;\n";

// `key` and `ref` are reserved by the JSX runtime; tsc removes them from the
// props check rather than treating them as excess.
#[test]
fn reserved_attributes_are_not_excess_properties() {
    let diagnostics = check(&format!(
        "{SETUP}export const a = <Item key=\"1\" task=\"x\" />;\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_genuinely_unknown_attribute_still_reports() {
    let diagnostics = check(&format!(
        "{SETUP}export const a = <Item nope=\"1\" task=\"x\" />;\n"
    ));
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}
