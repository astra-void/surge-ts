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

// A generic call re-resolves the declaration's written annotations under the
// call's substitution. That is not a fresh declaration check — the declaration
// was checked where it was written, against the type parameter's *constraint* —
// so nothing raised there belongs to the call. `(o: K[1])` under `K = [""]`
// reported a false TS2493 on a parameter list the call never looked at.
#[test]
fn an_indexed_access_on_a_type_parameter_is_not_rechecked_at_the_call() {
    let diagnostics = check(
        "declare function at<K extends [string, number?]>(k: K, o: K[1]): void;\n\
         at([\"\"], undefined);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn the_same_holds_for_a_nested_callback_parameter() {
    let diagnostics = check(
        "const wrap = <K extends [string, Record<string, unknown>?]>(\n\
             k: K,\n\
             fetcher: (o: K[1]) => void,\n\
         ) => [k, fetcher];\n\
         export const r = wrap([\"\"], () => {});\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// tsc does not check a type parameter's indexed access against the arity of its
// tuple constraint either — `K[1]` under `K extends [string]` resolves rather
// than reporting — so the suppression costs nothing here.
#[test]
fn an_index_outside_the_tuple_constraint_reports_nowhere() {
    let diagnostics = check(
        "declare function at<K extends [string]>(k: K, o: K[1]): void;\n\
         at([\"\"], undefined);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A written out-of-range index on a *concrete* tuple is the diagnostic tsc does
// emit, and it still does.
#[test]
fn a_written_out_of_range_tuple_index_still_reports() {
    let diagnostics = check(
        "type Pair = [string];\n\
         export type Second = Pair[1];\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2493"]);
}
