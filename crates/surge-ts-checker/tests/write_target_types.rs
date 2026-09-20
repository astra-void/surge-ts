//! What a write is checked against, following tsc's write path: the setter's
//! type for an accessor pair (`getWriteTypeOfSymbol`), the union of a union
//! receiver's members, a tuple's element at a literal index (with its bounds
//! errors), and `isReadonlySymbol` before any of it.

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

const ACCESSORS: &str = "class C {\n\
     \x20 #v = 0;\n\
     \x20 get value(): number { return this.#v; }\n\
     \x20 set value(next: number | string) { this.#v = Number(next); }\n\
     \x20 get only(): string { return \"x\"; }\n\
     \x20 readonly fixed: number = 1;\n\
     }\n\
     const c = new C();\n";

#[test]
fn a_write_uses_the_setter_type_not_the_getter_type() {
    let diagnostics = check(&format!("{ACCESSORS}c.value = \"ok\";\nc[\"value\"] = \"ok\";\n"));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(&format!("{ACCESSORS}c.value = true;\n"));
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert!(
        diagnostics[0].message.contains("string | number"),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn a_readonly_member_reports_before_assignability() {
    for write in ["c.fixed = 2;", "c[\"fixed\"] = 2;", "c.only = \"y\";", "c[\"only\"] = \"y\";"] {
        let diagnostics = check(&format!("{ACCESSORS}{write}\n"));
        assert_eq!(codes(&diagnostics), vec!["TS2540"], "{write}");
    }
}

#[test]
fn readonly_arrays_and_tuples_report_their_own_codes() {
    let diagnostics = check(
        "declare const ro: readonly number[];\n\
         export function f() { ro[0] = 1; }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2542"]);

    let diagnostics = check(
        "declare const rt: readonly [number, string];\n\
         export function f() { rt[0] = 1; }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2540"]);
}

#[test]
fn a_tuple_index_outside_its_length_reports_bounds_and_undefined() {
    let diagnostics = check(
        "const t: [number, string] = [1, \"a\"];\n\
         t[2] = 1;\n\
         export { t };\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2493", "TS2322"]);

    let diagnostics = check(
        "const t: [number, string] = [1, \"a\"];\n\
         t[-1] = 1;\n\
         export { t };\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2514", "TS2322"]);
}

#[test]
fn a_union_receiver_writes_against_the_union_of_its_members() {
    let diagnostics = check(
        "declare const u: number[] | string[];\n\
         export function f() { u[0] = true; }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);

    let diagnostics = check(
        "declare const u: number[] | string[];\n\
         export function f() { u[0] = 1; }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

/// `x op= v` is `x = x op v`: the operator's result type is what the write is
/// checked against. Before this, no compound assignment was parsed at all.
#[test]
fn compound_assignments_are_checked() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 const arr: number[] = [1];\n\
         \x20 arr[0] += 1;\n\
         \x20 arr[0] += \"s\";\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);

    let diagnostics = check(
        "export function f() {\n\
         \x20 let text = \"a\";\n\
         \x20 text += 1;\n\
         \x20 let total = 0;\n\
         \x20 total += \"s\";\n\
         \x20 return text + total;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn readonly_reaches_every_form_the_declaration_can_take() {
    for source in [
        "interface R { readonly x: number }\ndeclare const r: R;\nr.x = 1;\n",
        "type T = { readonly x: number };\ndeclare const t: T;\nt.x = 1;\n",
        "declare const inline: { readonly x: number };\ninline.x = 1;\n",
        "class C { constructor(public readonly x: number) {} }\nconst c = new C(1);\nc.x = 2;\nexport { c };\n",
        "const frozen = { x: 1 } as const;\nfrozen.x = 2;\nexport { frozen };\n",
        "interface G<T> { readonly x: T }\ndeclare const g: G<number>;\ng.x = 1;\n",
    ] {
        let diagnostics = check(source);
        assert_eq!(codes(&diagnostics), vec!["TS2540"], "{source}");
    }
}

#[test]
fn an_as_const_array_is_a_readonly_tuple() {
    let diagnostics = check(
        "const values = [1, 2] as const;\n\
         values[0] = 3;\n\
         export { values };\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2540"]);

    // …and is not assignable to a mutable array, which has its own code.
    let diagnostics = check(
        "const values = [1, 2] as const;\n\
         export const mutable: number[] = values;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS4104"]);
}

/// The elements keep their literal types through `ReadonlyArray<T>`: the const
/// context is what the assertion was written for, and a tuple's element type is
/// the union of its elements rather than two candidates that collapse.
#[test]
fn inference_through_readonly_array_keeps_the_literal_elements() {
    let diagnostics = check(
        "declare function firstOf<T>(values: ReadonlyArray<T>): T;\n\
         const modes = [\"fast\", \"slow\"] as const;\n\
         export const mode: \"fast\" | \"slow\" = firstOf(modes);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_generic_accessor_resolves_its_write_type_under_the_type_arguments() {
    let diagnostics = check(
        "class Box<T> {\n\
         \x20 #v: T;\n\
         \x20 constructor(v: T) { this.#v = v; }\n\
         \x20 get value(): T { return this.#v; }\n\
         \x20 set value(next: T | string) { this.#v = next as T; }\n\
         }\n\
         const b = new Box<number>(1);\n\
         b.value = \"from string\";\n\
         b.value = 2;\n\
         export { b };\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

/// tsc intersects the constituents' write types for a union key, so two members
/// of different types leave `never`.
#[test]
fn a_union_key_writes_against_the_intersection_of_what_it_names() {
    let diagnostics = check(
        "declare const o: { a: number; b: string };\n\
         declare const k: \"a\" | \"b\";\n\
         export function f() { o[k] = 1; }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert!(
        diagnostics[0].message.contains("'never'"),
        "{}",
        diagnostics[0].message
    );

    let diagnostics = check(
        "declare const o: { a: number; b: number };\n\
         declare const k: \"a\" | \"b\";\n\
         export function f() { o[k] = 1; }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

/// `AccessFlags.NoIndexSignatures`: a generic receiver indexed by a concrete
/// key can only be read. A generic key defers to an indexed-access type
/// instead, and a literal one names a member of the constraint.
#[test]
fn a_generic_receiver_can_only_be_indexed_for_reading() {
    let diagnostics = check(
        "export function write<T extends Record<string, number>>(o: T, k: string) {\n\
         \x20 o[k] = 1;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2862"]);

    let diagnostics = check(
        "export function writeGenericKey<T, K extends keyof T>(o: T, k: K, v: T[K]) {\n\
         \x20 o[k] = v;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(
        "export function writeNamed<T extends { a: number }>(o: T) {\n\
         \x20 o[\"a\"] = 1;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
