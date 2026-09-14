//! `receiver[index] = value` was dropped by the parser — only a *static*
//! member expression reached the checker — so no write through an element
//! access was ever checked.

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
fn array_element_writes_are_checked() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 const arr: number[] = [];\n\
         \x20 arr[0] = 1;\n\
         \x20 arr[1] = \"s\";\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn tuple_element_writes_use_the_element_at_that_index() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 const t: [number, string] = [1, \"a\"];\n\
         \x20 t[0] = 2;\n\
         \x20 t[1] = \"b\";\n\
         \x20 t[0] = \"s\";\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn index_signature_writes_are_checked_through_any_key() {
    let diagnostics = check(
        "export function f(key: string) {\n\
         \x20 const rec: Record<string, number> = {};\n\
         \x20 rec[\"a\"] = 1;\n\
         \x20 rec[key] = \"s\";\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn a_nested_element_write_resolves_like_the_matching_read() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 const nested: { inner: { v: number } } = { inner: { v: 1 } };\n\
         \x20 nested[\"inner\"][\"v\"] = \"s\";\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

/// A receiver whose element type surge cannot name stays unchecked rather than
/// being held to a guess.
#[test]
fn writes_through_an_unmodelled_receiver_are_not_reported() {
    let diagnostics = check(
        "export function f(anything: any, key: string) {\n\
         \x20 anything[key] = 1;\n\
         \x20 const loose: unknown = {};\n\
         \x20 (loose as any)[key] = \"s\";\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

/// tsc resolves the *value* before deciding an instantiation is abstract; the
/// name-keyed type lookup reported every `new Class()` in a function whose
/// parameter shadowed an abstract class of that name. zod declares
/// `abstract class Class` and passes a `Class` parameter into the same module.
#[test]
fn a_binding_that_shadows_an_abstract_class_is_not_an_abstract_instantiation() {
    let diagnostics = check(
        "export abstract class Class { constructor(public n: number) {} }\n\
         type Ctor = { new (n: number): { n: number } };\n\
         export function f(Class: Ctor) {\n\
         \x20 const rec: Record<string, { n: number }> = {};\n\
         \x20 rec[\"k\"] = new Class(1);\n\
         \x20 return rec;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(
        "export abstract class Class { constructor(public n: number) {} }\n\
         export function g() { return new Class(1); }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2511"]);
}
