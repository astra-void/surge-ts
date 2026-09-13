use surge_ts_checker::check_source;

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check(source_text: &str) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_source(source_text, "example.ts")
}

// tsc reports one error per object literal: a written property that fails is
// reported at that property, and the missing-required-property report never
// happens. The order matters beyond the message — a test writes
// `@ts-expect-error` over the property, which covers the property's line and
// not the literal's.
#[test]
fn a_failing_property_is_reported_instead_of_a_missing_one() {
    let diagnostics = check(
        "interface Opts { key: string; init: number; fn?: () => number }\n\
         declare function take(o: Opts): void;\n\
         declare const wrong: symbol;\n\
         take({ key: \"k\", fn: wrong });\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn the_missing_property_is_reported_when_every_written_one_matches() {
    let diagnostics = check(
        "interface Opts { key: string; init: number; fn?: () => number }\n\
         declare function take(o: Opts): void;\n\
         take({ key: \"k\", fn: () => 1 });\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2741"]);
}
