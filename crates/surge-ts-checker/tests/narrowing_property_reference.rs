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

// `if (o.p)` narrows the property inside the pushed then-branch scope, the way a
// bare-identifier truthy guard already did.
#[test]
fn truthy_property_guard_narrows_the_then_branch() {
    let diagnostics = check(
        "interface O { p?: string }\n\
         declare function want(s: string): void;\n\
         export function f(o: O): void {\n\
             if (o.p) {\n\
                 want(o.p);\n\
             }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The guard reaches through a nested path, not just one property level.
#[test]
fn truthy_property_guard_narrows_a_nested_path() {
    let diagnostics = check(
        "interface O { nested: { r?: string } }\n\
         declare function want(s: string): void;\n\
         export function f(o: O): void {\n\
             if (o.nested.r) {\n\
                 want(o.nested.r);\n\
             }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `this.p` is a reference like any other; `this` is bound as a symbol.
#[test]
fn truthy_this_property_guard_narrows_the_then_branch() {
    let diagnostics = check(
        "declare function want(s: string): void;\n\
         export class K {\n\
             p?: string;\n\
             m(): void {\n\
                 if (this.p) {\n\
                     want(this.p);\n\
                 }\n\
             }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Every operand of an `&&` holds in the then-branch, so both properties narrow.
#[test]
fn truthy_property_guard_narrows_across_an_and_chain() {
    let diagnostics = check(
        "interface O { p?: string; q?: string }\n\
         declare function want(s: string): void;\n\
         export function f(o: O): void {\n\
             if (o.p && o.q) {\n\
                 want(o.p);\n\
                 want(o.q);\n\
             }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A ternary branches through the symbol-table narrowing path, not the scope stack.
#[test]
fn truthy_property_guard_narrows_a_ternary_true_branch() {
    let diagnostics = check(
        "interface O { p?: string }\n\
         declare function want(s: string): void;\n\
         export function f(o: O): void {\n\
             want(o.p ? o.p : \"x\");\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A `typeof` test on a property narrows the property, including the optional
// flag that carries its `undefined`.
#[test]
fn typeof_property_guard_narrows_both_branches() {
    let diagnostics = check(
        "interface O { p?: string }\n\
         declare function want(s: string): void;\n\
         export function f(o: O): void {\n\
             if (typeof o.p === \"string\") {\n\
                 want(o.p);\n\
             }\n\
             if (typeof o.p === \"undefined\") {\n\
                 return;\n\
             }\n\
             want(o.p);\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The falsy complement is deliberately unmodelled, but the guard must not leak
// out of its branch: the property is still optional after the `if` and in the
// `else`.
#[test]
fn truthy_property_guard_does_not_leak_out_of_its_branch() {
    let diagnostics = check(
        "interface O { p?: string }\n\
         declare function want(s: string): void;\n\
         export function after(o: O): void {\n\
             if (o.p) {\n\
             }\n\
             want(o.p);\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);

    let diagnostics = check(
        "interface O { p?: string }\n\
         declare function want(s: string): void;\n\
         export function otherwise(o: O): void {\n\
             if (o.p) {\n\
             } else {\n\
                 want(o.p);\n\
             }\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// A `||` proves nothing about either operand in the true branch.
#[test]
fn truthy_property_guard_does_not_narrow_across_an_or() {
    let diagnostics = check(
        "interface O { p?: string; q?: string }\n\
         declare function want(s: string): void;\n\
         export function f(o: O): void {\n\
             if (o.p || o.q) {\n\
                 want(o.q);\n\
             }\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// Guarding one property proves nothing about a sibling.
#[test]
fn truthy_property_guard_does_not_narrow_a_sibling_property() {
    let diagnostics = check(
        "interface O { p?: string; q?: string }\n\
         declare function want(s: string): void;\n\
         export function f(o: O): void {\n\
             if (o.p) {\n\
                 want(o.q);\n\
             }\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// `if (!x) { x = …; }` joins the branch end with the fall-through, so the
// default-an-optional idiom leaves the binding non-nullish afterwards.
#[test]
fn assignment_in_a_guard_block_joins_with_the_fall_through() {
    let diagnostics = check(
        "declare function make(): number;\n\
         export function f(x?: number): number {\n\
             if (!x) {\n\
                 x = make();\n\
             }\n\
             return x;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The join is a union of both edges, so an assignment that keeps the nullish
// member (or a condition that proves nothing) still reports.
#[test]
fn the_branch_join_does_not_widen_beyond_both_edges() {
    let diagnostics = check(
        "declare function maybe(): number | undefined;\n\
         export function f(x?: number): number {\n\
             if (!x) {\n\
                 x = maybe();\n\
             }\n\
             return x;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);

    let diagnostics = check(
        "declare function make(): number;\n\
         declare const cond: boolean;\n\
         export function f(x?: number): number {\n\
             if (cond) {\n\
                 x = make();\n\
             }\n\
             return x;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// A declared `unknown`/`void` local is assumed initialized (tsc's
// `AnyOrUnknown | Void` gate), so an unassigned read is not TS2454.
#[test]
fn unknown_and_void_locals_are_assumed_initialized() {
    let diagnostics = check(
        "export function f(): unknown {\n\
             let e: unknown;\n\
             return e;\n\
         }\n\
         export function g(): void {\n\
             let v: void;\n\
             return v;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A non-null assertion at the read site suppresses definite-assignment analysis
// for that reference, as tsc's `assumeInitialized` does.
#[test]
fn a_non_null_assertion_suppresses_definite_assignment_analysis() {
    let diagnostics = check(
        "export function f(): number {\n\
             let n: number;\n\
             return n!;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(
        "export function f(): number {\n\
             let n: number;\n\
             return n;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2454"]);
}
