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

// `if ("p" in x)` narrows the pushed then/else branch scopes, not just the
// symbol tables built for `&&` operands and ternaries.
#[test]
fn in_operator_narrows_both_if_branches() {
    let diagnostics = check(
        "type A = { input: string; output: string };\n\
         type B = { serialize: string };\n\
         function f(t: A | B): string {\n\
             if (\"input\" in t) {\n\
                 return t.input;\n\
             } else {\n\
                 return t.serialize;\n\
             }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The fall-through of an early-returning `in` guard sees the complement, not the
// matching member.
#[test]
fn in_operator_narrows_after_a_diverting_branch() {
    let diagnostics = check(
        "type A = { input: string; output: string };\n\
         type B = { serialize: string };\n\
         function f(t: A | B): A {\n\
             if (\"input\" in t) {\n\
                 return t;\n\
             }\n\
             return { input: t.serialize, output: t.serialize };\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Named aliases and interfaces reach narrowing as nominal references, and an
// alias to a union stays a nested union after peeling — both must be looked
// through or the guard narrows nothing (the trpc `Field | Fields` shape).
#[test]
fn in_operator_narrows_a_union_of_named_types() {
    let diagnostics = check(
        "type O1 = { in: string; key: string };\n\
         type O2 = { key: string; map: string };\n\
         type O3 = { args?: number };\n\
         type Field = O1 | O2 | O3;\n\
         interface Fields { fields: string }\n\
         declare const c: Field | Fields;\n\
         export function a(): string | number | undefined {\n\
             if (\"in\" in c) {\n\
                 return c.key;\n\
             }\n\
             if (\"fields\" in c) {\n\
                 return c.fields;\n\
             }\n\
             if (\"map\" in c) {\n\
                 return c.map;\n\
             }\n\
             return c.args;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A named alias to a union narrows even when it is the whole declared type,
// with no enclosing union node.
#[test]
fn in_operator_narrows_an_alias_to_a_union() {
    let diagnostics = check(
        "type O1 = { in: string };\n\
         type O2 = { map: string };\n\
         type Field = O1 | O2;\n\
         function f(v: Field): string {\n\
             if (\"map\" in v) {\n\
                 return v.map;\n\
             }\n\
             return v.in;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// tsc keeps a member whose property is OPTIONAL in the false branch — the key
// may legitimately be absent at runtime — so the else branch stays the full
// union and an own-member access on it is still reported.
#[test]
fn in_operator_keeps_an_optional_member_in_the_false_branch() {
    let diagnostics = check(
        "type WithOpt = { a?: string; z: number };\n\
         type WithoutA = { b: string };\n\
         function f(v: WithOpt | WithoutA): number {\n\
             if (\"a\" in v) {\n\
                 return v.z;\n\
             }\n\
             return v.z;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

// `!("p" in x)` narrows the opposite branch.
#[test]
fn negated_in_operator_narrows_the_then_branch() {
    let diagnostics = check(
        "type A = { input: string };\n\
         type B = { serialize: string };\n\
         function f(t: A | B): string {\n\
             if (!(\"input\" in t)) {\n\
                 return t.serialize;\n\
             }\n\
             return t.input;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// An `in` guard composed into an `&&` chain narrows the branch too.
#[test]
fn in_operator_narrows_through_an_and_chain() {
    let diagnostics = check(
        "type A = { input: string };\n\
         type B = { serialize: string };\n\
         function f(t: A | B, ok: boolean): string {\n\
             if (ok && \"input\" in t) {\n\
                 return t.input;\n\
             }\n\
             return \"\";\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
