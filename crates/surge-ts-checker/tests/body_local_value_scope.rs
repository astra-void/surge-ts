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
fn body_local_alias_may_query_a_body_local_value() {
    let diagnostics = check(
        "export function f() {\n\
             const schema = { name: \"x\" };\n\
             type Schema = typeof schema;\n\
             const use = (arg: Schema) => arg.name;\n\
             return use(schema);\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The alias is forced from a call's explicit type argument, not from a
// variable annotation — the body's value scope has to be visible wherever the
// declaration is first resolved from.
#[test]
fn body_local_alias_typeof_resolves_from_a_type_argument() {
    let diagnostics = check(
        "declare function expectTypeOf<T>(): void;\n\
         export function f() {\n\
             const schema = { name: \"x\" };\n\
             type Schema = typeof schema;\n\
             expectTypeOf<Schema>();\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A body-local alias may shadow the value it queries (zod's `type a = Infer<typeof a>`).
#[test]
fn body_local_alias_may_shadow_the_value_it_queries() {
    let diagnostics = check(
        "export function f() {\n\
             const a: { q: number } = { q: 1 };\n\
             type a = typeof a;\n\
             const branded = (_: a) => {};\n\
             branded({ q: 2 });\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn arrow_parameter_annotation_sees_enclosing_body_locals() {
    let diagnostics = check(
        "export function outer() {\n\
             const s = { a: 1 };\n\
             const g = (x: typeof s) => x.a;\n\
             return g(s);\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn typeof_still_reports_a_genuinely_missing_name() {
    let diagnostics = check(
        "export function f() {\n\
             type T = typeof missing;\n\
             const x: T = null as any;\n\
             return x;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

// A body-local value goes out of scope with its body; a sibling body must not
// see it.
#[test]
fn body_local_value_scope_does_not_leak_to_a_sibling_body() {
    let diagnostics = check(
        "export function a() {\n\
             const s = { v: 1 };\n\
             type S = typeof s;\n\
             const x: S = s;\n\
             return x;\n\
         }\n\
         export function b() {\n\
             type S2 = typeof s;\n\
             const y: S2 = null as any;\n\
             return y;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

// `class D extends Parent {}` over a value: the base type comes from the
// value's construct signature, so nothing is unresolved and the inherited
// members are visible.
#[test]
fn body_local_class_extending_a_body_local_value_resolves() {
    let diagnostics = check(
        "declare const Base: new (..._args: any[]) => { z: number };\n\
         export function f() {\n\
             const Parent = Base;\n\
             class Definition extends Parent {}\n\
             return new Definition().z;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn body_local_class_extending_a_parameter_value_resolves() {
    let diagnostics = check(
        "declare const Base: new (..._args: any[]) => { z: number };\n\
         export function f(Parent: typeof Base) {\n\
             class Definition extends Parent {}\n\
             return new Definition().z;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// zod's `$constructor`, whose base is a union of constructors surge does not
// model: the derived type stays open instead of reporting an unresolved name.
#[test]
fn body_local_class_extending_an_unmodelled_value_base_is_open() {
    let diagnostics = check(
        "declare const Cls: new (..._args: any[]) => { a: number };\n\
         declare const Other: new (..._args: any[]) => { b: string };\n\
         export function $constructor(flag: boolean) {\n\
             const Parent = flag ? Cls : Other;\n\
             class Definition extends Parent {}\n\
             return Definition;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The class's static type is built at its own statement position, so a read
// from an earlier closure sees a reserved placeholder rather than nothing.
#[test]
fn body_local_class_read_before_its_declaration_is_not_reported() {
    let diagnostics = check(
        "export function f() {\n\
             const make = () => new C();\n\
             class C { x = 1 }\n\
             return make().x;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The heritage fallback is heritage-only: a value named in an ordinary
// annotation position is still reported.
#[test]
fn a_value_used_as_a_plain_type_annotation_is_still_reported() {
    let diagnostics = check(
        "declare const Base: new (..._args: any[]) => { z: number };\n\
         const Parent = Base;\n\
         declare let x: Parent;\n\
         export { x };\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}
