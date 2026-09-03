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

// Without `exactOptionalPropertyTypes` an optional property's type includes
// `undefined`, so its contextual type must too: a conditional value is checked
// branch by branch, and the `undefined` branch has to be accepted.
#[test]
fn conditional_undefined_is_assignable_to_an_optional_callback_property() {
    let diagnostics = check(
        "interface Props { cb?: () => void }\n\
         declare function take(props: Props): void;\n\
         declare const flag: boolean;\n\
         take({ cb: flag ? () => {} : undefined });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn conditional_undefined_is_assignable_to_an_optional_value_property() {
    let diagnostics = check(
        "interface Props { n?: number }\n\
         declare function take(props: Props): void;\n\
         declare const flag: boolean;\n\
         take({ n: flag ? 1 : undefined });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The widening must not swallow a genuinely wrong branch.
#[test]
fn conditional_with_a_mistyped_branch_still_reports() {
    let diagnostics = check(
        "interface Props { n?: number }\n\
         declare function take(props: Props): void;\n\
         declare const flag: boolean;\n\
         take({ n: flag ? \"x\" : undefined });\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// A required property keeps rejecting `undefined`; optionality is what admits it.
#[test]
fn conditional_undefined_against_a_required_property_still_reports() {
    let diagnostics = check(
        "interface Props { n: number }\n\
         declare function take(props: Props): void;\n\
         declare const flag: boolean;\n\
         take({ n: flag ? 1 : undefined });\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// Widening the contextual type to `T | undefined` must not cost a callback its
// parameter types — that would surface as TS7006 on `ctx` under noImplicitAny.
#[test]
fn optional_callback_property_still_contextually_types_its_parameters() {
    let diagnostics = check(
        "interface Ctx { body: string }\n\
         interface Props { on?: (context: Ctx) => void }\n\
         declare function take(props: Props): void;\n\
         take({\n\
             on: (ctx) => {\n\
                 const body: string = ctx.body;\n\
                 void body;\n\
             },\n\
         });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
