//! The branch that excludes the one literal a variable already holds is dead:
//! tsc's flow type there is `never`, and `never` compares with everything.

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

#[test]
fn else_if_over_the_remaining_literal_is_not_a_comparison_error() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 let mode: \"omit\" | \"extend\" = \"omit\";\n\
         \x20 if (mode === \"omit\") { mode = \"extend\"; }\n\
         \x20 else if (mode === \"extend\") { mode = \"omit\"; }\n\
         \x20 return mode;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_genuinely_disjoint_comparison_is_still_reported() {
    let diagnostics = check(
        "export function f(mode: \"omit\" | \"extend\") {\n\
         \x20 if (mode === \"omit\") { return 0; }\n\
         \x20 return mode === \"nope\" ? 1 : 2;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2367"]);
}
