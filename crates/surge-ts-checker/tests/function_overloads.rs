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

// A *generic* group re-resolves its parameter annotations at every call, from
// the one signature the group keeps, which threw the folded parameter union
// away: an argument written for a later overload was reported against the
// first. tanstack-query's `useQuery({ queryKey, queryFn })` — whose first
// overload demands `initialData` — was six false positives from this.
#[test]
fn an_argument_matching_a_later_generic_overload_is_accepted() {
    let diagnostics = check(
        "declare function pick<A>(o: { a: A; init: string }): 1;\n\
         declare function pick<A>(o: { a: A }): 2;\n\
         export const a = pick({ a: 1 });\n\
         export const b = pick({ a: 1, init: \"x\" });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn the_later_generic_overload_may_be_reached_through_an_alias() {
    let diagnostics = check(
        "type Defined<A> = { a: A; init: string };\n\
         type Undefined<A> = { a: A };\n\
         declare function pick<A>(o: Defined<A>): 1;\n\
         declare function pick<A>(o: Undefined<A>): 2;\n\
         export const a = pick({ a: 1 });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Partially supplied explicit type arguments take the same path, with the rest
// filled from the declared defaults.
#[test]
fn explicit_type_arguments_reach_the_later_generic_overload() {
    let diagnostics = check(
        "declare function pick<A = unknown, B = string>(o: { a: A; init: B }): 1;\n\
         declare function pick<A = unknown, B = string>(o: { a: A }): 2;\n\
         export const a = pick<number>({ a: 1 });\n\
         export const b = pick<number, string>({ a: 1 });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The fold is permissive on parameters only: a property that matches no
// overload of the group must still report. (Which code and span surge picks is
// the permissive fold's, not tsc's TS2769 — only the line agrees.)
#[test]
fn a_property_matching_no_generic_overload_still_reports() {
    let diagnostics = check(
        "declare function pick<A>(o: { a: A; init: string }): 1;\n\
         declare function pick<A>(o: { a: A }): 2;\n\
         export const a = pick({ a: 1, init: 2 });\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// The kept signature's return survives the fold itself. Widening it to the
// group's union (or to `any`) would degrade every generic group's result;
// which overload's return a call gets is selection's job, below.
#[test]
fn the_fold_leaves_the_return_type_alone() {
    let diagnostics = check(
        "declare function pick<A>(o: { a: A; init: string }): { first: A };\n\
         declare function pick<A>(o: { a: A }): { second: A };\n\
         export const a: { first: number } = pick({ a: 1, init: \"x\" });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Overload *selection*. The arguments are checked once, against the permissive
// fold; with their types in hand the return type is the first overload's that
// accepts them, and the fold's when none does. No candidate is re-checked and
// no diagnostic is rolled back, which is what kept the earlier attempt off
// main.
#[test]
fn the_return_type_is_the_first_accepting_overloads() {
    let diagnostics = check(
        "declare function pick(o: { a: number; init: string }): 'first';\n\
         declare function pick(o: { a: number }): 'second';\n\
         export const second: 'second' = pick({ a: 1 });\n\
         export const first: 'first' = pick({ a: 1, init: 'x' });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_generic_group_selects_after_instantiation() {
    let diagnostics = check(
        "declare function pick<A>(o: { a: A; init: string }): { first: A };\n\
         declare function pick<A>(o: { a: A }): { second: A };\n\
         export const inferred: { second: number } = pick({ a: 1 });\n\
         export const explicit: { second: number } = pick<number>({ a: 1 });\n\
         export const first: { first: number } = pick({ a: 1, init: 'x' });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn an_unannotated_binding_carries_the_selected_return() {
    let diagnostics = check(
        "declare function pick(o: { a: number; init: string }): 'first';\n\
         declare function pick(o: { a: number }): 'second';\n\
         const picked = pick({ a: 1 });\n\
         export const check: 'second' = picked;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A wrong pick is worse than no pick, so selection is stricter than the
// fold's assignability where tsc is: a weak object type (all properties
// optional) accepts nothing that shares no property with it. `readFileSync(p,
// 'utf8')` picked the `Buffer` overload without this.
#[test]
fn a_weak_object_parameter_does_not_capture_a_primitive() {
    let diagnostics = check(
        "declare function read(p: string, o?: { encoding?: null; flag?: string } | null): 1;\n\
         declare function read(p: string, o: { encoding: 'utf8'; flag?: string } | 'utf8'): 2;\n\
         export const text: 2 = read('x', 'utf8');\n\
         export const buffer: 1 = read('x');\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// An object literal's written keys are known from the syntax alone, so an
// overload requiring a property the literal never writes is rejected even
// when the literal's type could not be trusted. tanstack-query's
// `useQuery<string, Error>({ queryKey, queryFn })` was typed by the
// `initialData` overload's result without this, and every narrowing on it was
// wrong.
#[test]
fn a_required_property_the_literal_never_writes_rejects_the_overload() {
    let diagnostics = check(
        "declare function pick<A>(o: { a: A; init: string; cb?: () => A }): 'first';\n\
         declare function pick<A>(o: { a: A; cb?: () => A }): 'second';\n\
         export const second: 'second' = pick({ a: 1, cb: () => 1 });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A callback is typed by whichever overload is picked, so it cannot pick: it
// is a wildcard, and the other arguments decide.
#[test]
fn a_callback_argument_is_a_wildcard() {
    let diagnostics = check(
        "declare function run(kind: 'a', cb: (x: number) => void): 'first';\n\
         declare function run(kind: 'b', cb: (x: string) => void): 'second';\n\
         export const second: 'second' = run('b', (x) => x.length);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// When no overload accepts the arguments the call keeps the fold's return, so
// a no-match call reports exactly what it did before.
#[test]
fn no_accepting_overload_keeps_the_fold() {
    let diagnostics = check(
        "declare function widen(v: string): string;\n\
         declare function widen(v: number): number;\n\
         export const a = widen(true);\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}
