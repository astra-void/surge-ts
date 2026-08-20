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

#[test]
fn body_local_type_alias_resolves_in_annotations() {
    let diagnostics = check(
        "interface Wrap<T> { v: T; }\n\
         export function outer() {\n\
             type TError = Wrap<string>;\n\
             const a: TError = null as any;\n\
             function inner(o?: TError): TError { return o!; }\n\
             return { a, inner };\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn body_local_interface_and_class_resolve() {
    let diagnostics = check(
        "export function f() {\n\
             class Bar { x = 1 }\n\
             interface Baz { y: number }\n\
             const z: Baz = { y: 2 };\n\
             return new Bar().x + z.y;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A body-local alias may name the enclosing function's type parameters. They
// are not declarations, so they are seeded as degradation placeholders rather
// than reported as unresolved names.
#[test]
fn body_local_alias_over_outer_type_parameter_is_clean() {
    let diagnostics = check(
        "interface ClientError<T> { code: string; router: T; }\n\
         export function makeHooks<TRouter>() {\n\
             type TError = ClientError<TRouter>;\n\
             const e: TError = null as any;\n\
             function use(x: TError): TError { return x; }\n\
             return { e, use };\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn body_local_alias_may_reference_a_preceding_sibling() {
    let diagnostics = check(
        "export function f() {\n\
             type A = { n: number };\n\
             type B = { a: A };\n\
             const b: B = { a: { n: 1 } };\n\
             return b.a.n;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn body_local_type_does_not_escape_the_body() {
    let diagnostics = check(
        "function f() {\n\
             type Local = { p: string };\n\
             return null as any as Local;\n\
         }\n\
         const bad: Local = { p: \"x\" };\n\
         export { f, bad };\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

// Two sibling bodies may declare the same local name with different bodies;
// the program-wide resolution caches are keyed on the declaration name, so the
// two must not collapse onto one entry.
#[test]
fn same_named_body_local_aliases_stay_independent() {
    let diagnostics = check(
        "export function a() {\n\
             type Q = { p: string };\n\
             const v: Q = { p: 1 };\n\
             return v;\n\
         }\n\
         export function b() {\n\
             type Q = { p: number };\n\
             const v: Q = { p: 1 };\n\
             return v;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn body_local_declarations_still_report_assignability_errors() {
    let diagnostics = check(
        "export function f() {\n\
             type T = { a: number };\n\
             const x: T = { a: \"wrong\" };\n\
             return x;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn block_local_type_alias_resolves() {
    let diagnostics = check(
        "export function h(c: boolean) {\n\
             if (c) {\n\
                 type Inner = { q: string };\n\
                 const i: Inner = { q: \"a\" };\n\
                 return i.q;\n\
             }\n\
             return \"\";\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn generic_body_local_alias_accepts_type_arguments() {
    let diagnostics = check(
        "export function f() {\n\
             type P<X> = { v: X };\n\
             const p: P<number> = { v: 1 };\n\
             return p.v;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn body_local_class_extending_an_outer_class_is_constructible() {
    let diagnostics = check(
        "class Parent { constructor(..._a: any[]) {} }\n\
         export function f() {\n\
             class Definition extends Parent {}\n\
             return new Definition();\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
