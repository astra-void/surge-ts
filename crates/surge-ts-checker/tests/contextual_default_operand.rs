use surge_ts_checker::{CheckerOptions, check_source_with_options};

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check(source_text: &str) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_source_with_options(
        source_text,
        "example.ts",
        CheckerOptions {
            no_implicit_any: true,
            ..Default::default()
        },
    )
}

// `override ?? ((options) => …)` — the tRPC/react-query option-default shape.
// The annotation's contextual type reaches the right operand, so the fallback
// arrow's parameters are typed instead of implicit any.
#[test]
fn nullish_coalescing_right_operand_takes_the_annotations_contextual_type() {
    let diagnostics = check(
        "type OnSuccess = (options: { originalFn: () => void }) => void;\n\
         declare const override: OnSuccess | undefined;\n\
         const onSuccess: OnSuccess = override ?? ((options) => options.originalFn());\n\
         void onSuccess;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn logical_or_right_operand_takes_the_annotations_contextual_type() {
    let diagnostics = check(
        "type OnSuccess = (options: { originalFn: () => void }) => void;\n\
         declare const override: OnSuccess | undefined;\n\
         const onSuccess: OnSuccess = override || ((options) => options.originalFn());\n\
         void onSuccess;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The contextual type still has to be honoured: a right operand that does not
// fit the annotation is reported rather than silently accepted.
#[test]
fn nullish_coalescing_right_operand_reports_a_mismatch_against_the_annotation() {
    let diagnostics = check(
        "declare const override: number | undefined;\n\
         const value: number = override ?? \"fallback\";\n\
         void value;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// The left operand keeps contributing its non-nullish part to the result.
#[test]
fn nullish_coalescing_result_still_unions_the_non_nullish_left_operand() {
    let diagnostics = check(
        "declare const maybe: number | undefined;\n\
         const value: number | string = maybe ?? \"fallback\";\n\
         const narrowed: number = value;\n\
         void narrowed;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}
