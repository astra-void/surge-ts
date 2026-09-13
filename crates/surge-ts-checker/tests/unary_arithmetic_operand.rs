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

// TS2356 is the `++`/`--` operand rule. Unary `+`/`-` coerce, and tsc accepts
// any operand: reporting it here made `+data` on a contextually-typed `string`
// parameter a false positive (tanstack-query's `(data) => [data, +data]`).
#[test]
fn unary_plus_and_minus_accept_a_string() {
    let diagnostics = check(
        "declare const s: string;\n\
         export const a = +s;\n\
         export const b = -s;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn unary_plus_accepts_an_object_and_a_boolean() {
    let diagnostics = check(
        "declare const o: object;\n\
         declare const b: boolean;\n\
         export const x = +o;\n\
         export const y = +b;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn the_coerced_result_is_a_number() {
    let diagnostics = check(
        "declare const s: string;\n\
         export const n: number = +s;\n\
         export const m: string = -s;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn a_contextually_typed_parameter_coerces() {
    let diagnostics = check(
        "type Sel = (data: string) => [string, number];\n\
         export const sel: Sel = (data) => [data, +data];\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The operand is still an expression: dropping the report must not stop it
// being checked.
#[test]
fn the_operand_is_still_checked() {
    let diagnostics = check(
        "type Plain = { value: string };\n\
         declare const g: Plain;\n\
         export const a = +g.missing;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}
