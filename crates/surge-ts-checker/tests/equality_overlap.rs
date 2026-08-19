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

// tsc's equality rule accepts a nullable operand outright, so comparing any
// value to `undefined` is never TS2367.
#[test]
fn comparison_against_undefined_is_never_reported() {
    let diagnostics = check(
        "interface SchemaObject { type?: string }\n\
         declare const schema: SchemaObject;\n\
         declare const count: number;\n\
         declare const marker: symbol;\n\
         export const a = schema !== undefined;\n\
         export const b = count === undefined;\n\
         export const c = marker === undefined;\n\
         export const d = undefined === count;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `symbol`, `bigint`, arrays, tuples and named types all compare with
// themselves; the old whitelist had no arm for any of them.
#[test]
fn reflexive_comparisons_have_overlap() {
    let diagnostics = check(
        "interface Seen { a: number }\n\
         declare const s1: Seen;\n\
         declare const s2: Seen;\n\
         declare const sym1: symbol;\n\
         declare const sym2: symbol;\n\
         declare const big1: bigint;\n\
         declare const big2: bigint;\n\
         declare const arr1: string[];\n\
         declare const arr2: string[];\n\
         declare const tup1: [number, string];\n\
         declare const tup2: [number, string];\n\
         export const a = s1 === s2;\n\
         export const b = sym1 === sym2;\n\
         export const c = big1 === big2;\n\
         export const d = arr1 === arr2;\n\
         export const e = tup1 === tup2;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A union member that degraded to `unknown` must not make the whole comparison
// look disjoint — the trpc `T | typeof marker` sentinel shape.
#[test]
fn a_union_containing_unknown_overlaps_with_anything() {
    let diagnostics = check(
        "const marker = Symbol();\n\
         export function once<T>(fn: () => T): () => T {\n\
             let result: T | typeof marker = marker;\n\
             if (result === marker) {\n\
                 result = fn();\n\
             }\n\
             return () => result as T;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Provably disjoint primitive kinds keep reporting, including across a callable
// operand (the shape the whitelist inversion must not swallow).
#[test]
fn disjoint_operands_still_report() {
    for source in [
        "declare const n: number;\nexport const a = n === \"x\";\n",
        "export const a = \"x\" === 1;\n",
        "declare const f: (n: number) => void;\nexport const a = f === 204;\n",
        "declare const b: boolean;\nexport const a = b === 3;\n",
        "declare const s: symbol;\ndeclare const n: number;\nexport const a = s === n;\n",
        "declare const g: bigint;\ndeclare const s: string;\nexport const a = g === s;\n",
        "declare const arr: string[];\ndeclare const n: number;\nexport const a = arr === n;\n",
    ] {
        let diagnostics = check(source);
        assert_eq!(
            codes(&diagnostics),
            vec!["TS2367"],
            "expected TS2367 for {source:?}"
        );
    }
}

// Element and item types decide array/tuple overlap; same shape compares, a
// different element type does not.
#[test]
fn array_and_tuple_overlap_follows_their_elements() {
    let diagnostics = check(
        "declare const strings: string[];\n\
         declare const numbers: number[];\n\
         export const a = strings === numbers;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2367"]);

    let diagnostics = check(
        "declare const pair: [number];\n\
         declare const strings: string[];\n\
         export const a = pair === strings;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2367"]);
}

// Every non-nullish value is assignable to `{}`, so an empty object type
// compares with anything — but `void` is still a kind of its own.
#[test]
fn an_empty_object_type_overlaps_but_void_does_not() {
    let diagnostics = check(
        "declare const empty: {};\n\
         declare const count: number;\n\
         declare const fn: (n: number) => void;\n\
         export const a = empty === count;\n\
         export const b = empty === fn;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(
        "declare const nothing: void;\n\
         declare const count: number;\n\
         export const a = nothing === count;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2367"]);
}

// tsc exempts boolean and numeric literal conditions from the always-truthy /
// always-falsy checks, so the idiomatic `while (true)` stays clean.
#[test]
fn literal_boolean_and_numeric_conditions_are_not_flagged() {
    let diagnostics = check(
        "export function f() {\n\
             while (true) {\n\
                 break;\n\
             }\n\
             if (1) {\n\
             }\n\
             if (0) {\n\
             }\n\
             if (false) {\n\
             }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The empty string is always falsy, so it is TS2873 rather than TS2872.
#[test]
fn string_literal_conditions_pick_the_right_truthiness_code() {
    let diagnostics = check("export function f() {\n    if (\"abc\") {\n    }\n}\n");
    assert_eq!(codes(&diagnostics), vec!["TS2872"]);

    let diagnostics = check("export function f() {\n    if (\"\") {\n    }\n}\n");
    assert_eq!(codes(&diagnostics), vec!["TS2873"]);
}
