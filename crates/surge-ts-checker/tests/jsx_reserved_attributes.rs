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

const SETUP: &str = "declare global { namespace JSX { interface Element {} interface IntrinsicElements { div: {} } interface IntrinsicAttributes { key?: string | number } } }\n\
     interface Props { task: string }\n\
     declare function Item(props: Props): JSX.Element;\n";

// `key` is accepted because `JSX.IntrinsicAttributes` declares it, not because
// tsc reserves the name: without that interface it is an excess property.
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
