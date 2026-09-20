use surge_ts_checker::{
    CheckerOptions,
    check_source, check_source_with_options,
};

use super::*;

fn no_implicit_returns_options(no_implicit_returns: bool) -> CheckerOptions {
    CheckerOptions {
        no_implicit_returns,
        ..Default::default()
    }
}

#[test]
fn no_implicit_returns_reports_ts7030_on_partial_value_return() {
    let source = "export function a(x: number) { if (x > 0) return 1; }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7030"]);
}

#[test]
fn no_implicit_returns_reports_ts7030_on_arrow_block_body() {
    let source = "export const f = (x: number) => { if (x > 0) return 1; };";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7030"]);
}

#[test]
fn no_implicit_returns_silent_when_flag_off() {
    let source = "export function a(x: number) { if (x > 0) return 1; }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_returns_options(false));
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_when_all_paths_return() {
    let source = "export function b(x: number) { if (x > 0) return 1; return 2; }";
    let diagnostics = check_source_with_options(source, "b.ts", no_implicit_returns_options(true));
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_when_all_paths_exit_without_value() {
    // Every path exits explicitly (`return 1` / bare `return;`), so there is no
    // implicit fall-through — tsc emits nothing here even under noImplicitReturns.
    let source = "export function c(x: number) { if (x > 0) return 1; return; }";
    let diagnostics = check_source_with_options(source, "c.ts", no_implicit_returns_options(true));
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_on_infinite_loop() {
    // `while (true)` with no `break` never falls through, so the end is
    // unreachable and tsc emits no TS7030. (surge's separate always-truthy note
    // for `while (true)` is out of scope here, so assert only TS7030's absence.)
    let source = "export function g(x: number) { while (true) { if (x > 0) return 1; } }";
    let diagnostics = check_source_with_options(source, "g.ts", no_implicit_returns_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS7030"),
        "unexpected TS7030: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_on_throw_only_body() {
    // A function that only throws (no `return <value>`) infers a `void` return
    // type; tsc skips TS7030. A `throw` must not count as a value return.
    let source = "export function p(x: number) { if (x > 0) { throw x; } }";
    let diagnostics = check_source_with_options(source, "p.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_implicit_returns_silent_on_exhaustive_switch_with_default() {
    // Every clause returns and a `default` makes the switch exhaustive, so no
    // path falls through — tsc emits no TS7030.
    let source =
        "export function s(x: number) { switch (x) { case 1: return 'a'; default: return 'b'; } }";
    let diagnostics = check_source_with_options(source, "s.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_implicit_returns_reports_switch_without_default() {
    // No `default`: the discriminant may match no clause and fall through, so
    // tsc reports TS7030.
    let source =
        "export function s(x: number) { switch (x) { case 1: return 'a'; case 2: return 'b'; } }";
    let diagnostics = check_source_with_options(source, "s.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7030"]);
}

#[test]
fn no_implicit_returns_does_not_fire_on_constructor() {
    // tsc never applies noImplicitReturns to constructors (they implicitly
    // return `this`). A constructor with a conditional `return` value must not
    // produce TS7030.
    let source =
        "export class C { constructor(x: number) { if (x > 0) { return; } this.y = x; } y = 0; }";
    let diagnostics = check_source_with_options(source, "c.ts", no_implicit_returns_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS7030"),
        "unexpected TS7030: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_on_try_return_catch_throw() {
    // Every path exits: the `try` returns, the `catch` throws. The construct
    // never falls through, so tsc emits no TS7030 (regression: surge's Try flow
    // summary used to hardcode `guarantees_exit = false`).
    let source = "export function h(x: number) { try { return x; } catch (e) { throw e; } }";
    let diagnostics = check_source_with_options(source, "h.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_implicit_returns_does_not_affect_annotated_missing_return() {
    // Annotated return type still routes through TS2366, independent of the flag.
    let source = "export function e(x: number): number { if (x > 0) return 1; }";
    let diagnostics = check_source_with_options(source, "e.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS2366"]);
}

fn no_fallthrough_options(no_fallthrough_cases_in_switch: bool) -> CheckerOptions {
    CheckerOptions {
        no_fallthrough_cases_in_switch,
        ..Default::default()
    }
}

#[test]
fn no_fallthrough_reports_ts7029_on_reachable_clause_end() {
    let source = "export function a(x: number) { switch (x) { case 1: x; case 2: return; default: return; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7029"]);
}

#[test]
fn no_fallthrough_allows_empty_stacked_labels() {
    let source =
        "export function a(x: number) { switch (x) { case 1: case 2: return; default: return; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_fallthrough_allows_terminated_clauses() {
    let source = "export function a(x: number) { switch (x) { case 1: return; case 2: throw x; default: break; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_fallthrough_silent_when_flag_off() {
    let source = "export function a(x: number) { switch (x) { case 1: x; case 2: return; default: return; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(false));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS7029"),
        "got {:?}",
        codes(&diagnostics)
    );
}

fn no_implicit_override_options(no_implicit_override: bool) -> CheckerOptions {
    CheckerOptions {
        no_implicit_override,
        ..Default::default()
    }
}

#[test]
fn no_implicit_override_reports_ts4114_on_missing_override() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { greet(): string { return 'b'; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS4114"]);
}

#[test]
fn no_implicit_override_silent_with_override_modifier() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { override greet(): string { return 'b'; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_override_silent_on_new_member() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { extra(): number { return 1; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_override_silent_on_abstract_member_implementation() {
    // Implementing an abstract base member does not require `override`.
    let source = "abstract class Base { abstract run(): number; } class Impl extends Base { run(): number { return 1; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_override_reports_transitively() {
    let source = "class A { f(): number { return 1; } } class B extends A {} class C extends B { f(): number { return 2; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS4114"]);
}

#[test]
fn no_implicit_override_silent_when_flag_off() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { greet(): string { return 'b'; } }";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_implicit_override_options(false));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

fn no_property_access_index_options(
    no_property_access_from_index_signature: bool,
) -> CheckerOptions {
    CheckerOptions {
        no_property_access_from_index_signature,
        ..Default::default()
    }
}

#[test]
fn no_property_access_from_index_signature_reports_ts4111_on_dot_access() {
    let source = "interface D { [k: string]: number; } declare const d: D; const a = d.foo;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS4111"]);
}

#[test]
fn no_property_access_from_index_signature_allows_declared_property() {
    let source = "interface D { [k: string]: number; declared: number; } declare const d: D; const a = d.declared;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_allows_bracket_access() {
    let source = "interface D { [k: string]: number; } declare const d: D; const a = d[\"foo\"];";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_silent_without_index_signature() {
    let source = "interface P { x: number; } declare const p: P; const a = p.x;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_silent_for_synthetic_intersection_openness() {
    // `T & { url: string }` degrades the bare type parameter, and the merge keeps
    // the survivor open with a synthetic `any` string index so the dropped
    // operand's members are not reported as excess properties. That synthetic
    // index is not a declared index signature, so TS4111 must not fire on it.
    let source = "function use<T extends { path?: string }>(o: T & { url: string }) { return o.path; }";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_silent_for_degraded_heritage_base() {
    // A base that degrades (`T & { url: string }` drops the bare type parameter)
    // carries a synthetic open index. A derived interface inherits that
    // openness, not a declared index signature, so an undeclared member on the
    // derived type is still silent rather than a false TS4111.
    let source = "\
type Loose<T> = T & { url: string };
interface Derived<T extends { path?: string }> extends Loose<T> { own: number }
function use<T extends { path?: string }>(o: Derived<T>) { return [o.own, o.path, o.missing]; }";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_silent_when_flag_off() {
    let source = "interface D { [k: string]: number; } declare const d: D; const a = d.foo;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(false));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

fn ts6133_program_codes(source: &str, no_unused_locals: bool) -> Vec<String> {
    let options = CheckerOptions {
        no_unused_locals,
        ..Default::default()
    };
    let diagnostics = program_with_options(&[("a.ts", source)], options);
    codes(&diagnostics)
        .into_iter()
        .filter(|code| code == "TS6133")
        .collect()
}

#[test]
fn no_unused_locals_reports_unused_const() {
    let source = "export {};\nconst unused = 1;\n";
    assert_eq!(ts6133_program_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_locals_reports_unused_function() {
    let source = "export {};\nfunction unused(): number { return 1; }\n";
    assert_eq!(ts6133_program_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_locals_exempts_unused_class() {
    // An unused class is TS6196 ("declared but never used"), not TS6133.
    let source = "export {};\nclass Unused {}\n";
    assert!(ts6133_program_codes(source, true).is_empty());
    let options = CheckerOptions {
        no_unused_locals: true,
        ..Default::default()
    };
    assert_eq!(
        codes(&program_with_options(&[("a.ts", source)], options)),
        vec!["TS6196"]
    );
}

#[test]
fn no_unused_locals_exempts_used_and_exported() {
    let source = "const used = 1;\nexport const reexported = 2;\nexport const x = used;\n";
    assert!(ts6133_program_codes(source, true).is_empty());
}

fn ts6196_program_codes(source: &str) -> Vec<String> {
    let options = CheckerOptions {
        no_unused_locals: true,
        ..Default::default()
    };
    let diagnostics = program_with_options(&[("a.ts", source)], options);
    codes(&diagnostics)
        .into_iter()
        .filter(|code| code == "TS6196")
        .collect()
}

#[test]
fn no_unused_locals_reports_unused_body_local_type_alias() {
    let source = "export function f() {\n  type Unused = string;\n  return 1;\n}\n";
    assert_eq!(ts6196_program_codes(source), vec!["TS6196"]);
}

#[test]
fn no_unused_locals_reports_unused_body_local_interface() {
    let source = "export function f() {\n  interface Unused { a: string }\n  return 1;\n}\n";
    assert_eq!(ts6196_program_codes(source), vec!["TS6196"]);
}

#[test]
fn no_unused_locals_exempts_used_body_local_type_alias() {
    let source =
        "export function f() {\n  type Used = string;\n  const v: Used = \"a\";\n  return v;\n}\n";
    assert!(ts6196_program_codes(source).is_empty());
}

#[test]
fn no_unused_locals_ignores_scripts() {
    // No import/export: a script, whose top-level bindings are globals, not locals.
    let source = "const topLevel = 1;\n";
    assert!(ts6133_program_codes(source, true).is_empty());
}

#[test]
fn no_unused_locals_silent_when_flag_off() {
    let source = "export {};\nconst unused = 1;\n";
    assert!(ts6133_program_codes(source, false).is_empty());
}

#[test]
fn no_unused_locals_reports_function_local() {
    let source = "export function f(): number { const unused = 1; const used = 2; return used; }";
    assert_eq!(ts6133_program_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_locals_counts_local_read_in_nested_block_and_closure() {
    let source = "export function f(): number { const a = 1; const b = 2; if (a > 0) { return a; } return [b].map(x => x)[0]; }";
    assert!(ts6133_program_codes(source, true).is_empty());
}

fn no_unused_parameters_options(no_unused_parameters: bool) -> CheckerOptions {
    CheckerOptions {
        no_unused_parameters,
        ..Default::default()
    }
}

fn ts6133_codes(source: &str, on: bool) -> Vec<String> {
    let diagnostics = check_source_with_options(source, "a.ts", no_unused_parameters_options(on));
    codes(&diagnostics)
        .into_iter()
        .filter(|code| code == "TS6133")
        .collect()
}

#[test]
fn no_unused_parameters_reports_ts6133() {
    let source = "export function f(a: number, b: number): number { return b; }";
    assert_eq!(ts6133_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_parameters_exempts_underscore_prefix() {
    let source = "export function f(_a: number, b: number): number { return b; }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_counts_read_in_nested_function() {
    // The parameter is read only inside a nested function declaration — the oxc
    // read-walk must see it, so no TS6133.
    let source =
        "export function f(p: number): void { function inner(): number { return p; } inner(); }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_counts_read_in_template_literal() {
    let source = "export function f(token: string): string { return `Bearer ${token}`; }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_counts_read_in_spread() {
    let source = "export function f(p: number[]): number[] { return [...p]; }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_skips_overload_signatures() {
    // The bodyless overload signature's parameters must not be flagged; only the
    // implementation is checked (and here `a` is used).
    let source = "export function f(a: string): string;\nexport function f(a: number): string;\nexport function f(a: string | number): string { return String(a); }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_silent_when_flag_off() {
    let source = "export function f(a: number, b: number): number { return b; }";
    assert!(ts6133_codes(source, false).is_empty());
}

fn unused_parameters_options() -> CheckerOptions {
    CheckerOptions {
        no_unused_parameters: true,
        ..Default::default()
    }
}

#[test]
fn possibly_undefined_named_receiver_reports_ts18048() {
    let source = "declare const box: { value: number } | undefined; const v = box.value;";
    let diagnostics = check_source(source, "a.ts");
    assert_eq!(codes(&diagnostics), vec!["TS18048"]);
    assert!(
        diagnostics[0].message.contains("'box' is possibly 'undefined'"),
        "got {}",
        diagnostics[0].message
    );
}

#[test]
fn possibly_undefined_call_receiver_reports_ts2532() {
    let source = "declare function lookup(): { value: number } | undefined; const v = lookup().value;";
    let diagnostics = check_source(source, "a.ts");
    assert_eq!(codes(&diagnostics), vec!["TS2532"]);
}

#[test]
fn possibly_undefined_receiver_silent_after_guard_and_in_optional_chain() {
    let source = "declare const box: { value: number; inner?: { deep: string } } | undefined; \
        const a = box ? box.value : 0; const b = box?.value; const c = box!.value; \
        const d = box?.inner?.deep;";
    let diagnostics = check_source(source, "a.ts");
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn possibly_undefined_receiver_reports_optional_link_inside_chain() {
    let source =
        "declare const box: { inner?: { deep: string } } | undefined; const d = box?.inner.deep;";
    let diagnostics = check_source(source, "a.ts");
    assert_eq!(codes(&diagnostics), vec!["TS18048"]);
    assert!(
        diagnostics[0].message.contains("'box.inner' is possibly 'undefined'"),
        "got {}",
        diagnostics[0].message
    );
}

#[test]
fn all_unused_destructured_parameters_collapse_to_ts6198() {
    let source = "export const f = ({ a, b }: { a: number; b: number }) => 1;";
    let diagnostics = check_source_with_options(source, "a.ts", unused_parameters_options());
    assert_eq!(codes(&diagnostics), vec!["TS6198"]);
}

#[test]
fn partially_unused_destructured_parameters_report_each_ts6133() {
    let source = "export const f = ({ a, b }: { a: number; b: number }) => a;";
    let diagnostics = check_source_with_options(source, "a.ts", unused_parameters_options());
    assert_eq!(codes(&diagnostics), vec!["TS6133"]);
    assert!(diagnostics[0].message.contains("'b'"), "got {}", diagnostics[0].message);
}

#[test]
fn renamed_underscore_destructured_parameter_is_exempt() {
    let source = "export const f = ({ a: _a, b }: { a: number; b: number }) => b; \
        export const g = ([_x, y]: number[]) => y;";
    let diagnostics = check_source_with_options(source, "a.ts", unused_parameters_options());
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn unchecked_indexed_access_widens_array_reads() {
    let source = "declare const posts: { title: string }[]; const t = posts[0].title;";
    let diagnostics = check_source_with_options(
        source,
        "a.ts",
        CheckerOptions {
            no_unchecked_indexed_access: true,
            ..Default::default()
        },
    );
    assert_eq!(codes(&diagnostics), vec!["TS2532"]);
    let diagnostics = check_source(source, "a.ts");
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_property_access_from_index_signature_allows_destructuring() {
    let source = "interface D { [k: string]: number; } declare const d: D; const { foo } = d; const use = foo;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}
