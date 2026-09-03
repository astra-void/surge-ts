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

// `void` / `delete` / `~` have no modelled result, but their operands are still
// expressions: dropping the whole node stopped them being checked at all.
#[test]
fn void_operand_is_checked() {
    let diagnostics = check(
        "type Plain = { value: string };\n\
         declare const g: Plain;\n\
         void g.missing;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn void_operand_reports_an_unresolved_name() {
    let diagnostics = check("void nope;\n");
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn delete_operand_is_checked() {
    let diagnostics = check(
        "type Plain = { opt?: number };\n\
         declare const g: Plain;\n\
         delete g.missing;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn bitwise_not_operand_is_checked() {
    let diagnostics = check(
        "type Plain = { value: string };\n\
         declare const g: Plain;\n\
         const x = ~g.missing;\n\
         export { x };\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

// The result stays unmodelled: `void 0` must not start reporting as `undefined`,
// and a well-formed operand reports nothing.
#[test]
fn discarded_results_stay_unmodelled() {
    let diagnostics = check(
        "type Plain = { value: string; opt?: number };\n\
         declare const g: Plain;\n\
         declare const n: number;\n\
         const a = void 0;\n\
         const b = ~n;\n\
         delete g.opt;\n\
         void g.value;\n\
         export { a, b };\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
