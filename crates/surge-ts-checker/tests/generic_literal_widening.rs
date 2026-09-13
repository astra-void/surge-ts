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

const SUBJECT: &str = "declare function subject<T>(initial: T): { next: (v: T) => void; get: () => T };\n";

// `subject(1)` is `{ next: (v: number) => void }`, so a later `next(2)` fits.
#[test]
fn a_fresh_literal_argument_widens() {
    let diagnostics = check(&format!(
        "{SUBJECT}\
         export function f() {{\n\
             const value = subject(1);\n\
             value.next(2);\n\
             const s = subject(\"a\");\n\
             s.next(\"b\");\n\
             const b = subject(true);\n\
             b.next(false);\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The widened argument still types the result: assigning it elsewhere reports.
#[test]
fn the_widened_argument_still_types_the_result() {
    let diagnostics = check(&format!(
        "{SUBJECT}\
         export function f() {{\n\
             const value = subject(1);\n\
             value.next(\"nope\");\n\
         }}\n"
    ));
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// A constraint that asks for a literal keeps it.
#[test]
fn a_literal_constraint_keeps_the_literal() {
    let diagnostics = check(
        "declare function lit<T extends string>(v: T): T;\n\
         export const kept: \"x\" = lit(\"x\");\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_keyof_constraint_keeps_the_literal() {
    let diagnostics = check(
        "declare function pick<O, K extends keyof O>(o: O, k: K): O[K];\n\
         export const value: string = pick({ p: 1, q: \"s\" }, \"q\");\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A `const` assertion is not a fresh literal, so its literal types survive.
#[test]
fn a_const_asserted_argument_keeps_its_literals() {
    let diagnostics = check(
        "declare function id<T>(v: T): T;\n\
         export const kept: 1 = id({ n: 1 } as const).n;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A named alias for a literal union is indistinguishable from any other alias
// without resolving it, so a constrained parameter never widens: widening this
// one collapsed `Field` to `string` and made `Row[Field]` an invalid index.
#[test]
fn a_named_alias_constraint_keeps_the_literal() {
    let diagnostics = check(
        "type Row = { name: string; age: number };\n\
         type Editable = \"name\" | \"age\";\n\
         declare function update<Field extends Editable>(field: Field, value: Row[Field]): void;\n\
         export function f() {\n\
             update(\"name\", \"x\");\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Two literal arguments for one parameter still meet at the base primitive.
#[test]
fn two_literal_arguments_meet_at_the_primitive() {
    let diagnostics = check(
        "declare function pair<T>(a: T, b: T): T;\n\
         export const value: number = pair(1, 2);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A fresh object literal widens the same way a fresh primitive does:
// `subject({ n: 1 })` binds `T` to `{ n: number }`, so a later `next({ n: 2 })`
// fits. A `const` assertion is not fresh and keeps its literals.
#[test]
fn a_fresh_object_literal_argument_widens() {
    let diagnostics = check(&format!(
        "{SUBJECT}\
         export function f() {{\n\
             const value = subject({{ n: 1 }});\n\
             value.next({{ n: 2 }});\n\
             const n: number = value.get().n;\n\
             const kept = subject({{ n: 1 }} as const);\n\
             const one: 1 = kept.get().n;\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Two object candidates for one parameter infer the union of them, as tsc
// does, so the second argument is not checked against the first. A union
// parameter (`b: T | undefined`) takes the same path: an argument matching no
// structured member is still an answer for the naked one.
#[test]
fn two_object_arguments_meet_at_their_union() {
    let diagnostics = check(
        "declare function eq<T extends Record<string, any>>(a: T, b: T | undefined): boolean;\n\
         export const same = eq({ a: 1 }, { a: 2 });\n\
         export const wider = eq({ a: 1 }, { a: 1, b: 2 });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A callback argument infers through a rest parameter's *element*:
// `(...args: any[]) => any` — vitest's `vi.fn()` — binds each expected
// parameter to `any`, as tsc does. Zipping positionally bound the first to
// `any[]` and the rest to nothing, which left `TVariables` at its `void`
// default and rejected every `mutate(1)`.
#[test]
fn a_rest_any_callback_binds_every_expected_parameter() {
    let diagnostics = check(
        "declare const anyFn: (...args: any[]) => any;\n\
         interface Opts<TData, TVariables> {\n\
             mutationFn?: (variables: TVariables) => Promise<TData>;\n\
             onSuccess?: (data: TData, variables: TVariables) => void;\n\
         }\n\
         declare function observe<TData = unknown, TVariables = void>(\n\
             options: Opts<TData, TVariables>,\n\
         ): { mutate: (variables: TVariables) => Promise<TData> };\n\
         export function f() {\n\
             const observer = observe({ mutationFn: () => Promise.resolve('data'), onSuccess: anyFn });\n\
             observer.mutate(1);\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A callable object — an interface with a call signature — infers through
// that signature the way a function does.
#[test]
fn a_callable_object_infers_through_its_call_signature() {
    let diagnostics = check(
        "interface Mock { (...args: any[]): any; calls: number }\n\
         declare const mock: Mock;\n\
         declare function on<T>(handler: (value: T) => void): T;\n\
         export const value: string = on(mock);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
