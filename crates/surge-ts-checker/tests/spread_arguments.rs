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

// A spread supplies as many arguments as its type holds, so counting it as one
// made every `f(...tuple)` a false TS2554.
#[test]
fn a_spread_tuple_satisfies_the_arity() {
    let diagnostics = check(
        "declare function three(a: string, b: number, c: boolean): void;\n\
         const args = [\"a\", 1, true] as const;\n\
         export function f() { three(...args); }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_spread_array_into_a_rest_parameter_is_accepted() {
    let diagnostics = check(
        "declare const parts: string[];\n\
         declare function joinAll(...pieces: string[]): string;\n\
         export function f() { return joinAll(...parts); }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The spread's own expression is code: it was dropped as `Unknown` at parse
// time, so nothing inside it was ever checked.
#[test]
fn a_spread_expression_is_still_checked() {
    let diagnostics = check(
        "declare function three(a: string, b: number, c: boolean): void;\n\
         export function f() { three(...missingName); }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn a_spread_of_a_missing_member_is_still_checked() {
    let diagnostics = check(
        "type Holder = { items: [string, number] };\n\
         declare const holder: Holder;\n\
         declare function two(a: string, b: number): void;\n\
         export function f() { two(...holder.missing); }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

// A call with no spread still reports a real arity mismatch.
#[test]
fn a_plain_call_still_reports_a_wrong_argument_count() {
    let diagnostics = check(
        "declare function three(a: string, b: number, c: boolean): void;\n\
         export function f() { three(\"a\", 1); }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2554"]);
}

// Arguments beside a spread keep their own contextual check.
#[test]
fn an_argument_beside_a_spread_is_still_checked() {
    let diagnostics = check(
        "declare const rest: [number, boolean];\n\
         declare function three(a: string, b: number, c: boolean): void;\n\
         export function f() { three(1, ...rest); }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}
