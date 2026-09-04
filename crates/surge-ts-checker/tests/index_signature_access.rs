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

// A numeric key is converted to a string, so a string index signature answers it.
#[test]
fn a_numeric_key_reads_a_string_index_signature() {
    let diagnostics = check(
        "declare const rec: Record<string, boolean>;\n\
         declare const written: { [key: string]: boolean };\n\
         export const a: boolean = rec[1];\n\
         export const b: boolean = written[1];\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `Record<number, T>` is an index signature, not an empty property set.
#[test]
fn a_numeric_record_key_yields_an_index_signature() {
    let diagnostics = check(
        "declare const rec: Record<number, boolean>;\n\
         export const a: boolean = rec[1];\n\
         export const b: boolean = rec[0];\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_numeric_record_key_still_types_its_value() {
    let diagnostics = check(
        "declare const rec: Record<number, boolean>;\n\
         export const a: string = rec[1];\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// A literal-union key still enumerates properties, numeric literals included.
#[test]
fn a_literal_union_record_key_enumerates_properties() {
    let diagnostics = check(
        "declare const rec: Record<1 | 2, boolean>;\n\
         export const a: boolean = rec[1];\n\
         export const b = rec[3];\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

// A written mapped type takes the same rule as the built-in `Record` — the
// physical lib declares `Record<K, T>` as `{ [P in K]: T }`, so only this path
// runs there, and `number`/`symbol` keys collapsed the whole type to the
// sentinel.
#[test]
fn a_mapped_type_with_an_open_key_is_an_index_signature() {
    let diagnostics = check(
        "type ByKey<K extends string | number> = { [P in K]: boolean };\n\
         declare const byString: ByKey<string>;\n\
         declare const byNumber: ByKey<number>;\n\
         export const a: boolean = byString[\"k\"];\n\
         export const b: boolean = byNumber[1];\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_mapped_type_with_literal_keys_still_enumerates_them() {
    let diagnostics = check(
        "type ByKey<K extends string | number> = { [P in K]: boolean };\n\
         declare const byLiteral: ByKey<1 | 2>;\n\
         export const a: boolean = byLiteral[1];\n\
         export const b = byLiteral[3];\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

// A receiver with neither the member nor an index signature still reports.
#[test]
fn a_missing_numeric_member_still_reports() {
    let diagnostics = check(
        "interface Fixed { a: number }\n\
         declare const fixed: Fixed;\n\
         export const a = fixed[0];\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}
