use surge_ts_checker::CheckerOptions;

use super::*;

#[test]
fn exported_generic_constructor_preserves_dependent_defaults() {
    let diagnostics = program(&[
        ("model.ts", "export declare class Client<T = string, U = T> { value: U; }"),
        ("producer.ts", "import { Client } from './model'; export const client = new Client();"),
        ("consumer.ts", "import { client } from './producer'; const valid: string = client.value; const invalid: number = client.value; client.missing;"),
    ]);
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2339"]);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.file_name == "consumer.ts"));
}

#[test]
fn named_namespace_reexport_preserves_renamed_type_members() {
    let diagnostics = program(&[
        ("types.ts", "export namespace original { export interface Item { value: string; } }"),
        ("barrel.ts", "import { original as local } from './types'; export { local as renamed };"),
        ("consumer.ts", "import { renamed as ns } from './barrel'; const valid: ns.Item = { value: 'ok' }; const invalid: ns.Item = { value: 1 };"),
    ]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(diagnostics[0].file_name, "consumer.ts");
}

#[test]
fn explicit_predicate_type_arguments_narrow_the_subject() {
    let diagnostics = program(&[(
        "a.ts",
        "interface E<T> { data: T } declare function isE<T>(v: unknown): v is E<T>; export function f(x: { a: 1 } | E<string>) { if (isE<string>(x)) { const n: number = x.data; } }",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn written_call_signature_predicate_narrows() {
    let diagnostics = program(&[(
        "a.ts",
        "interface E { data: string } interface IsE { (v: unknown): v is E } declare const isE: IsE; export function f(x: { a: 1 } | E) { if (isE(x)) { const n: number = x.data; } }",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn asserts_condition_call_applies_the_predicate_inside() {
    let diagnostics = program(&[(
        "a.ts",
        "declare function assert(c: unknown, m?: string): asserts c; interface E { data: string } declare function isE(v: unknown): v is E; export function f(x: { a: 1 } | E) { assert(isE(x)); const n: number = x.data; }",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn predicate_over_unrelated_object_opens_the_subject() {
    let diagnostics = program(&[(
        "a.ts",
        "interface E { data: string } declare function isE(v: unknown): v is E; export function f(err: Error) { if (isE(err)) { const s: string = err.data; err.message; } }",
    )]);
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn nullish_empty_object_fallback_keeps_the_index_signature() {
    let diagnostics = program(&[(
        "a.ts",
        "declare const rec: Record<string, { x: number }> | undefined; const props = rec ?? {}; const v = props['data']; const s: string = v;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn interface_first_class_merge_keeps_the_class_heritage() {
    let diagnostics = program(&[(
        "a.ts",
        "class Base { off(): void {} listeners(): number[] { return []; } } declare interface CE<TOutput> { on(event: 'data', listener: (d: TOutput) => void): this; } class CE<TOutput> extends Base {} const ee = new CE<string>(); ee.on('data', (d: string) => {}); ee.off(); const n: number = ee.listeners(); const m: number = ee.on;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2322"]);
}

#[test]
fn never_indexed_access_is_silent_under_instantiation() {
    let diagnostics = program(&[(
        "a.ts",
        "type Shape<T> = (T extends string ? { errorShape: 1 } : never)['errorShape']; type X = Shape<number>; export const y: X = null!;",
    )]);
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn generic_interface_method_infers_from_its_arguments() {
    let diagnostics = program(&[(
        "a.ts",
        "class A2<T> { a!: T } interface VD { kind: 'vd' } interface Coll<N> { n: N; f1<T>(x: A2<T>): Coll<T>; f4<T>(x: T): Coll<T>; each(cb: (p: N) => void): this } declare const c: Coll<any>; declare const a2: A2<VD>; const n1: number = c.f1(a2).n; const n2: number = c.f4('x').n; c.f1(a2).each((p) => { const q: number = p; }); c.f1(a2).nope;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2322", "TS2322", "TS2339"]);
}

#[test]
fn generic_method_infers_through_a_union_alias_parameter() {
    let diagnostics = program(&[(
        "a.ts",
        "class A<T> { a!: T; check(v: any): v is T { return true; } } class B<T> { b!: T; check(v: any): v is T { return true; } } type Ty<T> = A<T> | B<T>; interface VD { kind: 'vd' } interface Trav { find<T extends object>(type: Ty<T>): Coll<T>; } interface Coll<N> extends Trav { nodes(): N[]; } declare const root: Coll<any>; declare const VD: Ty<VD>; const s: string = root.find(VD).nodes();",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert!(diagnostics[0].message.contains("VD[]"), "{}", diagnostics[0].message);
}

#[test]
fn optional_call_on_an_open_reference_is_silent() {
    let diagnostics = program(&[(
        "a.ts",
        "type Setdown<C1 extends object = object> = (context: Partial<C1>) => void; declare function run<T>(cb: Setdown<T>): void; run<any>(async (ctx) => { await ctx?.close?.(); });",
    )]);
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn member_call_predicate_narrows_a_property_reference() {
    let diagnostics = program(&[(
        "a.ts",
        "interface Checker<T> { check(v: any): v is T } interface A { kind: 'a' } interface M { object: A | M; name: string } declare const tm: Checker<M>; declare function f(m: M): boolean; export function g(expr: M): boolean { if (tm.check(expr.object)) { return f(expr.object); } return !tm.check(expr.object) ? false : f(expr.object); }",
    )]);
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn member_predicate_keeps_the_base_possibly_undefined() {
    let diagnostics = program_with_options(
        &[(
            "a.ts",
            "interface Checker<T> { check(v: any): v is T } interface Id { name: string } interface D { id: Id | number; init: string } declare const isId: Checker<Id>; export function f(ds: D[]) { const d = ds[0]; if (isId.check(d.id) && d.id.name === 'x') { d.init; } }",
        )],
        CheckerOptions {
            no_unchecked_indexed_access: true,
            ..CheckerOptions::default()
        },
    );
    assert_eq!(codes(&diagnostics), vec!["TS18048", "TS18048", "TS18048"]);
}

#[test]
fn write_to_a_narrowed_member_checks_the_declared_type() {
    let diagnostics = program(&[(
        "a.ts",
        "interface Id { name: string } interface Spec { imported: Id } export function f(spec: Spec) { if (spec.imported.name === 'a') { spec.imported.name = 'b'; } }",
    )]);
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn static_assertion_method_narrows_a_catch_variable() {
    let diagnostics = program_with_options(
        &[(
            "a.ts",
            "class E { issues: string[] = []; static assert(v: unknown): asserts v is E {} } export function f() { try { throw 1; } catch (err) { E.assert(err); const n: number = err.issues; } }",
        )],
        CheckerOptions {
            use_unknown_in_catch_variables: true,
            ..CheckerOptions::default()
        },
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn a_constructable_object_carries_the_function_prototype_member() {
    let diagnostics = program(&[(
        "a.ts",
        "declare const C: { new (): { a: 1 }; init(): void }; C.prototype; C.nope;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn strict_catch_variable_is_rejected_as_an_argument() {
    let diagnostics = program_with_options(
        &[(
            "a.ts",
            "interface Opts { a: 1 } declare function f(msg: string, o?: Opts): void; export function g() { try { throw 1; } catch (err) { f('x', err); } }",
        )],
        CheckerOptions {
            use_unknown_in_catch_variables: true,
            ..CheckerOptions::default()
        },
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

#[test]
fn an_import_type_query_reads_a_module_export() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "export declare const count: number;",
        "declare const c: typeof import('dep')['count']; const s1: string = c; declare const q: typeof import('dep').count; const s2: string = q; const s3: string = null as unknown as typeof import('dep')['count'];",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2322", "TS2322"]);
}

#[test]
fn an_import_type_query_global_assertion_narrows() {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), "node_modules/dep/index.d.ts".to_string());
    let diagnostics = program_with_options(
        &[
            (
                "node_modules/chai/index.d.ts",
                "declare namespace Chai { interface Assert { (expression: any, message?: string): asserts expression; } }",
            ),
            (
                "node_modules/dep/index.d.ts",
                "declare const assert: Chai.Assert; export { assert };",
            ),
            (
                "node_modules/dep/globals.d.ts",
                "declare global { let assert: typeof import('dep')['assert']; } export {};",
            ),
            (
                "src/index.ts",
                "interface E { data: number } declare function isE(v: unknown): v is E; export function f(err: unknown) { assert(isE(err)); const s: string = err.data; }",
            ),
        ],
        options,
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn a_rest_parameter_typed_by_a_type_parameter_infers_the_argument_tuple() {
    let diagnostics = program(&[(
        "a.ts",
        "declare function f<E extends any[]>(n: number, ...args: E): void; f(1, [1, 2]); f(2, 'a', [3]); f(3); declare function g<T>(...args: T[]): T; const s: string = g(1, 2); const t: number = g('a', 'b');",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2322"]);
}

#[test]
fn widening_a_mutable_binding_keeps_the_index_signature() {
    let diagnostics = program(&[(
        "a.ts",
        "export function m() { const c: { a: number; [k: string]: any } = { a: 1 }; let d = c; d.client; var e = c; e.client; let o = { a: 1 }; const s: string = o.a; }",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn parameters_of_optional_and_rest_parameters_are_optional_tuple_slots() {
    let diagnostics = program(&[(
        "a.ts",
        "class C { m(a?: { k: string[] }, b?: { x: number }): void {} n(a: string, ...rest: number[]): void {} } type P = Parameters<C['m']>; const p1: P = []; const p2: P = [{ k: ['1'] }]; const p3: Parameters<C['n']> = ['a']; const p4: P = [{ k: '1' }];",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn an_object_literal_against_a_tuple_reports_the_excess_property() {
    let diagnostics = program(&[(
        "a.ts",
        "const t: [string] = { k: 1 }; declare function f<T>(v: T): void; f<[string]>({ k: 1 });",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2353", "TS2353"]);
}

#[test]
fn an_object_literal_against_a_union_is_typed_by_the_per_property_unions() {
    let diagnostics = program(&[(
        "a.ts",
        "interface A { body?: string; headers?: [string, string][] | Record<string, string>; method?: string } interface B { body?: string | null; headers?: [string, string][] | Record<string, string>; method?: string; extra?: number } declare function f(init?: A | B): void; f({ body: '', headers: Math.random() > 0.5 ? [['a', 'b']] : { a: 'b' }, method: 'GET' }); f({ body: '', headers: 1 });",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

#[test]
fn a_type_parameter_bound_to_any_makes_the_extends_side_indeterminate() {
    let diagnostics = program(&[(
        "a.ts",
        "type Trouble<M extends string> = M & { _: symbol }; interface Inferrable { _config: unknown } declare function hook<T extends Inferrable>(opts: Inferrable extends T ? Trouble<'missing'> : { links: unknown[] }): void; declare const real: Inferrable & { x: 1 }; hook<typeof real>({ links: [] }); hook<typeof real>({ nope: [] }); declare const loose: any; hook<typeof loose>({ links: [] }); type Direct = Inferrable extends any ? 'yes' : 'no'; const s: number = null as unknown as Direct;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2353", "TS2322"]);
}

#[test]
fn constructing_a_generic_class_without_type_arguments_uses_the_constraints() {
    let diagnostics = program(&[(
        "a.ts",
        "class B<TContext extends object, TMeta extends object> { create(): { ctx: TContext; meta: TMeta } { return null as any; } } const b = new B(); b.nonexistent; const c: string = b.create(); class D<T = string> { get(): T { return null as any; } } const d: string = new D().get();",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2339", "TS2322"]);
}

#[test]
fn a_union_return_infers_through_a_conditional_alias_as_one_candidate() {
    let diagnostics = program(&[(
        "a.ts",
        "type UnsetMarker = 'unsetMarker' & { __brand: 'unsetMarker' }; type DefaultValue<TValue, TFallback> = UnsetMarker extends TValue ? TFallback : TValue; type MaybePromise<T> = T | Promise<T>; type Resolver<TCtx, TOutIn, $Output> = (opts: { ctx: TCtx }) => MaybePromise<DefaultValue<TOutIn, $Output>>; declare function mutation<$Output>(resolver: Resolver<{ a: 1 }, UnsetMarker, $Output>): $Output; const r1 = mutation(() => Math.random() > 0.5 ? { type: 'success' as const, id: 1 } : { type: 'error' as const, message: 'fail' }); const s1: string = r1; const r2 = mutation(() => Math.random() > 0.5 ? { id: 1 } : undefined); const s2: string = r2; declare function mut2<TOut>(resolver: () => TOut | Promise<TOut>): TOut; const r3 = mut2(() => Math.random() > 0.5 ? { id: 1 } : { name: 'x' }); const s3: string = r3;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2322", "TS2322"]);
}

