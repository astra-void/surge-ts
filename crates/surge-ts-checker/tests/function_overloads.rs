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

// The authoritative check pass re-registered every declaration and replaced,
// so only the last overload survived and a call matching an earlier one was a
// false TS2554.
#[test]
fn a_call_matching_an_earlier_overloads_arity_is_accepted() {
    let diagnostics = check(
        "declare function two(v: number): string;\n\
         declare function two(v: number, r: number): string;\n\
         export const a = two(1);\n\
         export const b = two(1, 2);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_call_matching_an_earlier_overloads_parameter_is_accepted() {
    let diagnostics = check(
        "declare function widen(v: string): string;\n\
         declare function widen(v: number): number;\n\
         export const a = widen(\"x\");\n\
         export const b = widen(1);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// An overload group merges permissively, so an argument matching *no* overload
// still reports — surge names the merged parameter where tsc reports TS2769.
#[test]
fn an_argument_matching_no_overload_still_reports() {
    let diagnostics = check(
        "declare function widen(v: string): string;\n\
         declare function widen(v: number): number;\n\
         export const a = widen(true);\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// Overload signatures in front of an implementation are the same group.
#[test]
fn overload_signatures_before_an_implementation_merge() {
    let diagnostics = check(
        "function impl(v: string): number;\n\
         function impl(v: string, r: string): number;\n\
         function impl(v: string, r?: string): number { return 1; }\n\
         export const a = impl(\"x\");\n\
         export const b = impl(\"x\", \"y\");\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Two *implementations* are a duplicate declaration, not an overload group.
#[test]
fn two_implementations_still_report_a_duplicate() {
    let diagnostics = check(
        "function dup(v: string): string { return v; }\n\
         function dup(v: string): string { return v; }\n\
         export const a = dup(\"x\");\n",
    );
    assert!(
        codes(&diagnostics).iter().any(|code| code == "TS2393"),
        "{:?}",
        codes(&diagnostics)
    );
}

// The group is per name, and a same-named function in another scope is a
// different binding: merging across them would silence a real mismatch.
#[test]
fn same_named_functions_in_separate_scopes_do_not_merge() {
    let diagnostics = check(
        "export function outerA() {\n\
             function shared(v: string): string { return v; }\n\
             return shared(1);\n\
         }\n\
         export function outerB() {\n\
             function shared(v: number): number { return v; }\n\
             return shared(\"a\");\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345", "TS2345"]);
}
