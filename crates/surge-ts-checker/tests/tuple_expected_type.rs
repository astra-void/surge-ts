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

// tsc names the source by its own widened element types; the hardcoded
// `unknown[]` this used to print named neither side truthfully.
#[test]
fn a_too_long_tuple_literal_names_its_own_type() {
    let diagnostics = check("export const pair: [string, number] = [\"Ada\", 36, true];\n");
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert!(
        diagnostics[0]
            .message
            .contains("Type '[string, number, boolean]' is not assignable to type '[string, number]'"),
        "{}",
        diagnostics[0].message
    );
}

// An element that does not resolve stands in as `any`, which is how tsc renders
// its error type — and the length mismatch is still one diagnostic, with no
// cascade from the unresolved element.
#[test]
fn an_unresolved_extra_element_reads_as_any() {
    let diagnostics = check("export const pair: [string, number] = [\"Ada\", 36, missing];\n");
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2304"]);
    assert!(
        diagnostics[0]
            .message
            .contains("Type '[string, number, any]'"),
        "{}",
        diagnostics[0].message
    );
}

// `T extends [A, ...A[]] | []` is the "one or more, or none" signature zod's
// `tuple` uses. The `[]` member only says the argument may also be empty, so the
// elements must be read from the non-empty member; without looking through the
// union the parameter stayed unsolved, landed on the empty tuple, and reported
// every well-formed argument.
#[test]
fn a_union_tuple_constraint_infers_from_its_non_empty_member() {
    let diagnostics = check(
        "type Item = { a: number };\n\
         declare const it: Item;\n\
         declare function tuple<T extends [Item, ...Item[]] | []>(v: T): T;\n\
         export const a = tuple([it, it]);\n\
         export const b = tuple([]);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_union_array_constraint_still_infers() {
    let diagnostics = check(
        "type Item = { a: number };\n\
         declare const it: Item;\n\
         declare function list<T extends Item[] | []>(v: T): T;\n\
         export const a = list([it, it]);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A tuple target still reports a genuine element mismatch.
#[test]
fn a_tuple_element_mismatch_still_reports() {
    let diagnostics = check("export const pair: [string, number] = [\"Ada\", \"36\"];\n");
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}
