use surge_ts_checker::{
    CheckerOptions, SourceFileInput, check_program_with_options,
    check_source,
};

use super::*;

#[test]
fn definite_assignment_assertion_skips_the_unassigned_check() {
    let source = concat!(
        "export function f() {\n",
        "  let t!: string;\n",
        "  return t;\n",
        "}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn destructuring_default_removes_undefined() {
    let source = concat!(
        "type P = { a?: number };\n",
        "declare const p: P;\n",
        "export function f() {\n",
        "  const { a = 0 } = p;\n",
        "  const x: number = a;\n",
        "  return x;\n",
        "}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn class_method_does_not_shadow_an_outer_function() {
    // A method name is a member, not a lexical binding: the bare call inside the
    // body must reach the module-level `helper`.
    let source = concat!(
        "export function helper(a: string, b: number): string {\n",
        "  return a;\n",
        "}\n",
        "export class C {\n",
        "  helper(a: string): string {\n",
        "    return helper(a, 1);\n",
        "  }\n",
        "}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn intersected_function_types_are_overloads() {
    let source = concat!(
        "type M = ((a: string) => number) & ((a: string, b: number) => string);\n",
        "declare const m: M;\n",
        "export const r = m(\"a\", 1);\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn interface_call_signature_overloads_merge() {
    let source = concat!(
        "interface M {\n",
        "  (a: string): number;\n",
        "  (a: string, b: number): string;\n",
        "}\n",
        "declare const m: M;\n",
        "export const r = m(\"a\", 1);\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn array_reduce_and_string_replace_type_their_callbacks() {
    let source = concat!(
        "declare const items: string[];\n",
        "export const a = items.reduce((acc, value) => acc + value.length, 0);\n",
        "declare const s: string;\n",
        "export const b = s.replace(/x/g, (c) => c.toUpperCase());\n",
        "export const c = s.normalize(\"NFC\");\n",
        "export const d = items.filter((x) => x);\n",
    );
    let options = CheckerOptions {
        no_implicit_any: true,
        strict_property_initialization: false,
        strict_null_checks: true,
        ..Default::default()
    };
    let diagnostics = program_with_options(&[("a.ts", source)], options);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn object_destructuring_binds_each_property_type() {
    // The binding used to receive the *source* type, so every use compared
    // against the whole object (`for (const { schema } of items) …`).
    let source = concat!(
        "const items = [{ a: 1, b: \"x\" }];\n",
        "export function f() {\n",
        "  for (const { a, b } of items) {\n",
        "    const bad: string = a;\n",
        "    return [bad, b];\n",
        "  }\n",
        "  return [];\n",
        "}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert!(
        diagnostics[0].message.contains("number"),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn ts_expect_error_suppresses_the_next_line() {
    let source = concat!(
        "export function need(a: string): void {}\n",
        "export function f() {\n",
        "  // @ts-expect-error\n",
        "  need();\n",
        "}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn ts_expect_error_does_not_suppress_other_lines() {
    let source = concat!(
        "export function need(a: string): void {}\n",
        "export function f() {\n",
        "  // @ts-expect-error\n",
        "  need();\n",
        "  need();\n",
        "}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(codes(&diagnostics), vec!["TS2554"]);
}

#[test]
fn namespace_member_generic_call_infers_type_arguments() {
    // The namespace value object models only the member *set*, so the call must
    // resolve through the qualified `ns.member` binding to infer `U`.
    let source = concat!(
        "export namespace util {\n",
        "  export const arrayToEnum = <T extends string, U extends [T, ...T[]]>(\n",
        "    items: U\n",
        "  ): { [k in U[number]]: k } => ({}) as any;\n",
        "}\n",
        "const codes = util.arrayToEnum([\"a\", \"b\"]);\n",
        "export const bad: 1 = codes.a;\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert!(
        diagnostics[0].message.contains('"') && diagnostics[0].message.contains('a'),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn namespace_member_generic_call_resolves_through_an_import() {
    let util = concat!(
        "export namespace util {\n",
        "  export function first<T>(items: T[]): T {\n",
        "    return items[0];\n",
        "  }\n",
        "}\n",
    );
    let consumer = concat!(
        "import { util } from \"./util.js\";\n",
        "export const bad: 1 = util.first([\"a\"]);\n",
    );
    let diagnostics = program(&[("util.ts", util), ("consumer.ts", consumer)]);
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn never_parameter_residual_argument_is_reported() {
    // Only `"a"` is narrowed away, so the residual `"b"` is not assignable to
    // `never`.
    let source = concat!(
        "export function assertNever(_x: never): never {\n",
        "  throw new Error();\n",
        "}\n",
        "export function f(value: \"a\" | \"b\"): string {\n",
        "  if (value === \"a\") return \"a\";\n",
        "  assertNever(value);\n",
        "}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

/// A JSX component whose props type could not be modelled offers no contextual
/// type for an inline callback attribute, so reporting implicit-any there would
/// describe surge's own gap rather than the source — the radix
/// `ComponentProps<typeof Primitive.Root>` cluster. A component with a real
/// props type still reports.
#[test]
fn jsx_callback_props_report_implicit_any() {
    let mut options = CheckerOptions::default();
    options.no_implicit_any = true;
    let diagnostics = program_with_options(
        &[(
            "src/index.tsx",
            "type Unmodelled = keyof number;\n\
             declare function Widget(props: Unmodelled): null;\n\
             export const a = <Widget onPick={(value) => value} />;\n",
        )],
        options,
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS7006"]);

    let mut options = CheckerOptions::default();
    options.no_implicit_any = true;
    let diagnostics = program_with_options(
        &[(
            "src/index.tsx",
            "declare function Widget(props: { label: string }): null;\n\
             export const a = <Widget onPick={(value) => value} />;\n",
        )],
        options,
    );
    assert!(
        codes(&diagnostics).contains(&"TS7006".to_string()),
        "{:?}",
        codes(&diagnostics)
    );
}

/// `keyof {}` is `never`, so the empty-interface escape hatch React uses for
/// `Key`/`ReactNode` (`T[keyof T]` over a members-less interface) contributes
/// nothing to its union instead of degrading it. A type surge could not model
/// still yields the `unknown` sentinel rather than a closed `never`.
#[test]
fn keyof_empty_interface_is_never() {
    let diagnostics = check_source(
        "interface Escape {}\n\
         type Key = string | number | Escape[keyof Escape];\n\
         declare const k: Key;\n\
         export const s: string | number = k;\n\
         export const bad: string = k;\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

/// `ComponentProps<typeof Component>` matches `JSXElementConstructor<infer P>`
/// against a nominal reference (`ForwardRefExoticComponent<Props>`) whose own
/// arguments are empty — the props live in its resolved call signature. The
/// positional reference shortcut has nothing to line up there, so it must fall
/// through to the structural expansion instead of leaving every capture at its
/// seeded placeholder (which collapsed the whole conditional to `unknown`).
#[test]
fn infer_capture_binds_through_argumentless_reference() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "interface Exotic { (props: { checked: boolean }): string }\n\
         type Ctor<P> = (props: P) => string;\n\
         type PropsOf<T extends Ctor<any>> = T extends Ctor<infer P> ? P : {};\n\
         declare const Widget: Exotic;\n\
         export { Widget, type PropsOf };\n",
        "import { Widget, type PropsOf } from 'dep';\n\
         declare const p: PropsOf<typeof Widget>;\n\
         export const ok: boolean = p.checked;\n\
         export const bad: string = p.checked;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn native_profile_suppresses_indexed_access_cascade() {
    let files = vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "
        interface User { name: string; }
        type UnresolvedKeyIndex = User[MissingKeyName];
        let _trigger: UnresolvedKeyIndex;
        "
        .to_string(),
    }];

    let mut options = CheckerOptions::default();
    // Default is Tsc profile
    let tsc_diagnostics = check_program_with_options(files.clone(), options.clone());
    assert_eq!(codes(&tsc_diagnostics), vec!["TS2304", "TS2538"]);

    options.diagnostic_profile = surge_ts_checker::DiagnosticProfile::Native;
    let native_diagnostics = check_program_with_options(files, options);
    assert_eq!(codes(&native_diagnostics), vec!["TS2304"]);
}

#[test]
fn indexed_access_unresolved_object_reports_only_missing_type() {
    let files = vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "
        type UnresolvedObjectIndex = MissingObject[\"x\"];
        let _trigger: UnresolvedObjectIndex;
        "
        .to_string(),
    }];

    let diagnostics = check_program_with_options(files, CheckerOptions::default());
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

// `infer` capture inside a function-parameter position resolves to the matched
// argument: the excess-property error proves `R` is the concrete `{ id: string }`
// props object, not a degraded `unknown`/`any`. Regression guard for the
// function-pattern arm of conditional `infer` binding (the core of
// `React.ComponentProps<typeof FunctionComponent>`).
#[test]
fn infer_capture_through_inline_function_parameter() {
    let diagnostics = check_source(
        "type FirstParam<T> = T extends (props: infer P) => any ? P : never;\n\
         declare const comp: (props: { id: string }) => void;\n\
         type R = FirstParam<typeof comp>;\n\
         const bad: R = { nope: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// `infer` capture reached by expanding a generic alias whose body is a function
// (`Ctor<infer P>`), matching React's `JSXElementConstructor<infer Props>` shape.
#[test]
fn infer_capture_through_generic_function_alias() {
    let diagnostics = check_source(
        "type Ctor<P> = (props: P) => unknown;\n\
         type PropsOf<T> = T extends Ctor<infer P> ? P : never;\n\
         declare const comp: (props: { id: string }) => unknown;\n\
         type R = PropsOf<typeof comp>;\n\
         const bad: R = { nope: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// A constructor signature (`new (...) => T`) as a union member must not collapse
// the whole union to `unknown`. This mirrors React's
// `JSXElementConstructor<P> = ((props: P) => …) | (new (props: P) => …)`; the
// props type is recovered from the call-signature member.
#[test]
fn constructor_type_union_member_preserves_call_signature_infer() {
    let diagnostics = check_source(
        "type ElementCtor<P> = ((props: P) => string) | (new (props: P) => object);\n\
         type PropsOf<T> = T extends ElementCtor<infer P> ? P : never;\n\
         declare const widget: (props: { title: string }) => string;\n\
         type R = PropsOf<typeof widget>;\n\
         const bad: R = { other: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// `infer` capture against a callable object (an interface carrying a call
// signature, e.g. React's `ForwardRefExoticComponent<P>`) resolves through the
// object's call signature rather than degrading.
#[test]
fn infer_capture_through_callable_interface_signature() {
    let diagnostics = check_source(
        "interface Callable { (props: { id: string }): void; }\n\
         declare const callable: Callable;\n\
         type FirstParam<T> = T extends (props: infer P) => any ? P : never;\n\
         type R = FirstParam<typeof callable>;\n\
         const bad: R = { nope: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// An arrow argument whose parameter type is a callable object (an interface
// carrying a call signature, e.g. React's `ForwardRefRenderFunction`) is
// contextually typed by that call signature rather than left implicit-any: `v`
// resolves to `number`, so assigning it to `string` is the only diagnostic.
#[test]
fn callable_object_parameter_contextually_types_arrow() {
    let diagnostics = check_source(
        "interface Render { (x: number): void; }\n\
         declare function take(fn: Render): void;\n\
         take((v) => { const s: string = v; });\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// A function value is assignable to a callable object target when it matches the
// target's call signature, and not when its parameter is incompatible — so only
// the mismatched call is rejected.
#[test]
fn function_assignable_to_callable_object_target() {
    let diagnostics = check_source(
        "interface Render { (x: number): void; }\n\
         declare function take(fn: Render): void;\n\
         const ok = (n: number): void => { void n; };\n\
         const bad = (s: string): void => { void s; };\n\
         take(ok);\n\
         take(bad);\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// A tuple-typed rest parameter (`...args: [name: string]`) — and a union of
// tuples, next's cookie-store overload shape — accepts each argument at its
// tuple position instead of comparing the whole tuple/union against every
// argument. Only the genuinely mismatched call reports.
#[test]
fn tuple_rest_parameter_accepts_positional_arguments() {
    let diagnostics = check_source(
        "declare function get(...args: [string] | [{ name: string }]): void;\n\
         get(\"NEXT_LOCALE\");\n\
         get({ name: \"lang\" });\n\
         get(123);\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// A spread of a nominally-typed source (`{ ...defaults, ...props }` where both
// are `Props`) contributes the reference's members instead of being skipped, so
// destructured names resolve rather than reporting TS2339 on `{}`.
#[test]
fn object_literal_spread_peels_nominal_reference() {
    let diagnostics = check_source(
        "interface Props { url: string; email: string }\n\
         const defaults: Props = { url: \"u\", email: \"e\" };\n\
         function render(props: Props = defaults): string {\n\
             const { url, email } = { ...defaults, ...props };\n\
             return url + email;\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A callable object used as a JSX component (a `forwardRef`/`memo`-style exotic
// component, which is callable rather than a bare function) has its props checked
// through its call signature.
#[test]
fn jsx_callable_object_component_checks_props() {
    let diagnostics = check_source(
        "interface Btn { (props: { label: string }): null; }\n\
         declare const Button: Btn;\n\
         const bad = <Button label={123} />;\n",
        "example.tsx",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// tsc never excess-checks hyphenated JSX attribute names (`data-slot`,
// `aria-*`), while a non-hyphenated unknown attribute still reports.
#[test]
fn jsx_hyphenated_attribute_is_not_excess_checked() {
    let diagnostics = check_source(
        "declare function Item(props: { label?: string }): null;\n\
         const ok = <Item data-slot=\"x\" aria-bogus=\"y\" />;\n\
         const bad = <Item dataslot=\"x\" />;\n",
        "example.tsx",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// A `{...spread}` whose type resolves to an object contributes its members, so
// a required prop carried by the spread is not reported missing (the shadcn
// wrapper idiom), while a spread without it still reports TS2741.
#[test]
fn jsx_spread_attributes_cover_required_props() {
    let diagnostics = check_source(
        "declare function Item(props: { label: string }): null;\n\
         declare const full: { label: string };\n\
         declare const partial: { id?: number };\n\
         const ok = <Item {...full} />;\n\
         const bad = <Item {...partial} />;\n",
        "example.tsx",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2741"]);
}

// Calling an imported generic with explicit type arguments re-resolves its
// declared parameter/return annotations; their names live in the declaring
// module's scope, not the caller's (react-hook-form's
// `useForm(props?: UseFormProps<…>): UseFormReturn<…>`), so the instantiation
// must run under the declaring file or the names report false TS2304s.
#[test]
fn imported_generic_call_resolves_signature_in_declaring_module_scope() {
    let diagnostics = program(&[
        (
            "lib.ts",
            "export interface Options<T> { seed: T }\n\
             export interface Box<T> { value: T }\n\
             export function make<T>(options?: Options<T>): Box<T> {\n\
                 return { value: (options as Options<T>).seed };\n\
             }\n",
        ),
        (
            "main.ts",
            "import { make } from \"./lib\";\n\
             const box = make<{ email: string }>();\n\
             const s: string = box.value.email;\n",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

// An opaque spread (`any`, unresolved) folds the whole attributes object into
// `any` in tsc, so both the missing-required and excess checks stand down.
#[test]
fn jsx_opaque_spread_suppresses_presence_checks() {
    let diagnostics = check_source(
        "declare function Item(props: { label: string }): null;\n\
         declare const rest: any;\n\
         const ok = <Item {...rest} bonus={1} />;\n",
        "example.tsx",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `import * as z from "./external"; export { z }` (zod's index.d.ts shape)
// re-exports the namespace's type members, so a type-only named import of `z`
// can reference them as qualified names instead of reporting TS2305.
#[test]
fn namespace_import_reexport_exposes_member_types() {
    let diagnostics = program(&[
        (
            "external.ts",
            "export interface Payload { value: string }\n",
        ),
        (
            "barrel.ts",
            "import * as z from \"./external\";\nexport { z };\n",
        ),
        (
            "main.ts",
            "import type { z } from \"./barrel\";\n\
             const p: z.Payload = { value: \"ok\" };\n\
             export default p;\n",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

// A JSX expression types as ReactElement<any, any> in tsc, so it satisfies a
// structurally-declared element shape (the react-hook-form `render` callback
// return); an empty opaque stub would miss the required members.
#[test]
fn jsx_element_satisfies_react_element_shape() {
    let diagnostics = check_source(
        "declare function take(render: () => { type: string; props: unknown; key: string | null }): void;\n\
         take(() => <div />);\n",
        "example.tsx",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

// A user-defined type predicate (`value is T`) narrows its bare-identifier
// argument in the guarded branch, composing with `&&` (`m && isDevice(m)`).
#[test]
fn type_predicate_call_narrows_guarded_identifier() {
    let diagnostics = check_source(
        "type Device = \"iphone\" | \"android\" | \"other\";\n\
         function isDevice(value: string): value is Device {\n\
             return value === \"iphone\";\n\
         }\n\
         function pick(m: string | undefined): Device {\n\
             if (m && isDevice(m)) return m;\n\
             return \"other\";\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The fall-through of an early-returning predicate guard removes the matching
// members (`if (isStr(x)) return …;` leaves `number`).
#[test]
fn type_predicate_false_branch_removes_matching_members() {
    let diagnostics = check_source(
        "function isStr(v: unknown): v is string {\n\
             return typeof v === \"string\";\n\
         }\n\
         function f(x: string | number): number {\n\
             if (isStr(x)) return x.length;\n\
             return x * 2;\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A `function` declaration nested in another function keeps its collected
// signature when hoisted into scope, so a sibling closure's predicate guard
// still narrows (the unnamed settings-page shape).
#[test]
fn nested_function_type_predicate_narrows_in_sibling_closure() {
    let diagnostics = check_source(
        "type Device = \"iphone\" | \"android\" | \"other\";\n\
         function outer(m: string | undefined): Device {\n\
             function isDevice(value: string): value is Device {\n\
                 return value === \"iphone\";\n\
             }\n\
             const get = (): Device => {\n\
                 if (m && isDevice(m)) return m;\n\
                 return \"other\";\n\
             };\n\
             return get();\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `find` on an `as const` tuple yields a flat `element | undefined` union that
// is assignable to (and truthy-narrows against) its written equivalent.
#[test]
fn tuple_find_result_is_flat_element_union() {
    let diagnostics = check_source(
        "const A = [\"x\", \"y\"] as const;\n\
         const found: \"x\" | \"y\" | undefined = A.find((d) => d.length > 0);\n\
         function pick(): \"x\" | \"y\" {\n\
             const inner = A.find((d) => d.length > 0);\n\
             if (inner) return inner;\n\
             return \"x\";\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `x as const satisfies T` keeps the const-asserted literal element types
// (plain `infer_expression` would widen them to `string`).
#[test]
fn const_satisfies_keeps_literal_elements() {
    let diagnostics = check_source(
        "type Device = \"iphone\" | \"android\" | \"other\";\n\
         const DEVICES = [\"iphone\", \"android\", \"other\"] as const satisfies Device[];\n\
         const first: Device = DEVICES[0];\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// tsc applies `noPropertyAccessFromIndexSignature` only to written dotted
// accesses; a destructuring binding from an index-signature type is allowed.
// An interface `get`/`set` accessor pair lowers to a property typed by the
// getter, the shape the DOM lib uses for `Window.location`
// (`get location(): Location; set location(href: string)`).
#[test]
fn interface_get_set_accessor_pair_reads_as_getter_type() {
    let diagnostics = check_source(
        "interface Loc { href: string }\n\
         interface Host {\n\
             get location(): Loc;\n\
             set location(href: string);\n\
         }\n\
         declare const host: Host;\n\
         const href: string = host.location.href;\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A conditional spread contributes optional properties, so `in`-guarded access
// on the spread member resolves (tsc infers `list?: string[]`).
#[test]
fn conditional_spread_yields_optional_properties() {
    let diagnostics = check_source(
        "declare const cond: boolean;\n\
         const step = { title: \"t\", ...(cond ? { list: [\"a\"] } : {}) };\n\
         const items: string[] | undefined = step.list;\n\
         const title: string = step.title;\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `Promise<T>` is modeled as its awaited `T`, so a chained `.then`/`.catch` must
// still resolve instead of reporting the member missing on the value type.
#[test]
fn then_chain_on_awaited_call_result_resolves() {
    let diagnostics = check_source(
        "async function load(): Promise<number | undefined> { return 1; }\n\
         export function use() {\n\
             return load().then(() => 2);\n\
         }\n\
         export async function useCatch() {\n\
             await load().catch(() => undefined);\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Assigning to a union-declared variable narrows it (the lazy-singleton idiom),
// but only within the block that assigned — a branch assignment must not leak.
//
// The `pick` half also pins literal-equality narrowing: `target` is
// `"draft-07"` on both edges into the second test, so tsc 7.0.2 reports
// `TS2367` there (verified against the oracle at the same file/code/line/column
// and message text). This assertion held `[]` while surge had no
// literal-equality narrowing at all.
#[test]
fn assignment_narrows_union_within_its_block_only() {
    let diagnostics = check_source(
        "interface Client { id: number }\n\
         declare function createClient(): Client;\n\
         let client: Client | null = null;\n\
         function get(): Client {\n\
             if (client) return client;\n\
             client = createClient();\n\
             return client;\n\
         }\n\
         type Target = \"draft-04\" | \"draft-07\";\n\
         function pick(input: Target): Target {\n\
             let target: Target = input;\n\
             if (target === \"draft-04\") target = \"draft-07\";\n\
             if (target === \"draft-04\") return target;\n\
             return target;\n\
         }\n\
         export const use = [get, pick];\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2367"]);
}

// An overload group folds into one callable shape: a call selecting a later
// overload is accepted rather than checked against the first signature only.
#[test]
fn overload_group_accepts_later_overload_argument() {
    let diagnostics = native_program(&[
        (
            "cache.d.ts",
            "export declare function cacheLife(profile: \"default\"): void;\n\
             export declare function cacheLife(profile: \"minutes\"): void;\n\
             export declare function cacheLife(profile: \"hours\"): void;",
        ),
        (
            "index.ts",
            "import { cacheLife } from \"./cache\";\n\
             cacheLife(\"minutes\");\n\
             cacheLife(\"hours\");",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `import type * as ns` elides only the runtime binding: `typeof ns.Member`
// stays legal in type positions.
#[test]
fn type_only_namespace_import_supports_typeof_member() {
    let diagnostics = native_program(&[
        (
            "primitive.ts",
            "export const Root = (props: { checked: boolean }) => props.checked;",
        ),
        (
            "index.ts",
            "import type * as Primitive from \"./primitive\";\n\
             type RootFn = typeof Primitive.Root;\n\
             declare const root: RootFn;\n\
             export const value: boolean = root({ checked: true });",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A destructuring callback parameter is only an error against a written
// `unknown`; a contextual type the checker failed to resolve must stay silent.
#[test]
fn destructured_callback_parameter_reports_only_written_unknown() {
    let written = check_source(
        "declare function accept(fn: (opts: unknown) => void): void;\n\
         accept(({ a }) => { console.log(a); });\n",
        "example.ts",
    );
    assert_eq!(codes(&written), vec!["TS2345"]);
}

// A numeric-literal property key keeps its member: dropping one collapsed the
// whole type literal to `unknown`, which then rejected the index
// (Prisma's generated `Not<B> = { 0: 1; 1: 0 }[B]`).
#[test]
fn numeric_literal_property_keys_are_indexable() {
    let diagnostics = check_source(
        "type Not<B extends 0 | 1> = { 0: 1; 1: 0 }[B];\n\
         declare const flipped: Not<0>;\n\
         const one: 1 = flipped;\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// An `infer` capture the pattern match cannot line up degrades instead of
// reporting the capture name as an unknown type in the true branch.
#[test]
fn unmatched_infer_capture_degrades_instead_of_reporting() {
    let diagnostics = check_source(
        "export type IntersectOf<U> = (U extends unknown ? (k: U) => void : never) extends (\n\
             k: infer I,\n\
         ) => void\n\
             ? I\n\
             : never;\n\
         declare const merged: IntersectOf<{ a: 1 } | { b: 2 }>;\n\
         export const use = merged;\n",
        "example.ts",
    );
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS2304"),
        "{:?}",
        codes(&diagnostics)
    );
}

// The declared name resolves inside a nested function body in its own
// initializer; a direct self-read stays a temporal-dead-zone error.
#[test]
fn declaration_name_visible_in_its_own_initializer_closure() {
    let diagnostics = check_source(
        "export function schedule() {\n\
             const timer = setInterval(() => { clearInterval(timer); }, 10);\n\
             return timer;\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let self_read = check_source(
        "export function f() {\n\
             const x = x + 1;\n\
             return x;\n\
         }\n",
        "example.ts",
    );
    assert!(!self_read.is_empty(), "self-read must still be reported");
}

// An overload group whose returns disagree because one did not resolve degrades
// rather than committing to the resolved one — `createElement("textarea")` must
// not read as the string overload's `HTMLElement`.
#[test]
fn overload_group_with_unresolved_return_degrades() {
    let diagnostics = native_program(&[
        (
            "dom.d.ts",
            "interface TagMap { textarea: { select(): void } }\n\
             interface Doc {\n\
                 make<K extends keyof TagMap>(tag: K): TagMap[K];\n\
                 make(tag: string): { id: string };\n\
             }\n\
             declare const doc: Doc;",
        ),
        ("index.ts", "doc.make(\"textarea\").select();"),
    ]);
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS2339"),
        "{:?}",
        codes(&diagnostics)
    );
}

// `declare namespace N { export { a, b } }` re-exports enclosing declarations as
// namespace members, so `N.a` resolves off a namespace import instead of
// reporting a missing property on an empty object (Prisma's runtime
// `Extensions`).
#[test]
fn namespace_export_list_contributes_value_members() {
    let diagnostics = native_program(&[
        (
            "runtime.d.ts",
            "declare function getExtensionContext<T>(that: T): T;\n\
             declare function defineExtension(args: number): string;\n\
             declare namespace Extensions {\n\
                 export { defineExtension, getExtensionContext }\n\
             }\n\
             export { Extensions }",
        ),
        (
            "index.ts",
            "import * as runtime from \"./runtime\";\n\
             export const ctx = runtime.Extensions.getExtensionContext;\n\
             export const def = runtime.Extensions.defineExtension;",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A quoted key names an object-literal property: dropping it inferred a
// fully-quoted literal (Prisma's generated client config) as `{}` and reported
// every required property as missing.
#[test]
fn quoted_object_literal_keys_are_members() {
    let diagnostics = check_source(
        "type Config = { previewFeatures: string[]; clientVersion: string };\n\
         const config: Config = {\n\
             \"previewFeatures\": [],\n\
             \"clientVersion\": \"7.8.0\",\n\
         };\n\
         export const use = config;\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The fall-through of `A || B` is `!A && !B`: a discriminant disjunct must be
// removed from the union, not selected. Treating the guard as one atom left the
// tested member in place.
#[test]
fn or_guard_fall_through_removes_the_tested_member() {
    let diagnostics = check_source(
        "type Row =\n\
             | { kind: \"resolved\"; warnings: string[] }\n\
             | { kind: \"unresolved\"; reason: string };\n\
         declare function fatal(): string | null;\n\
         export function f(rows: Row[]) {\n\
             for (const r of rows) {\n\
                 if (r.kind === \"unresolved\" || fatal()) { continue; }\n\
                 console.log(r.warnings.length);\n\
             }\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The same fall-through with an *early return* rather than `continue`: the
// composite-guard path never collected discriminant subjects, so `resolved`
// stayed the full union past the guard.
#[test]
fn or_guard_return_fall_through_removes_the_tested_member() {
    let diagnostics = check_source(
        "type Row =\n\
             | { kind: \"resolved\"; warnings: string[] }\n\
             | { kind: \"unresolved\"; reason: string };\n\
         export function f(r: Row, fatal: string): string[] {\n\
             if (r.kind === \"unresolved\" || fatal) { return []; }\n\
             return r.warnings;\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `x === null` / `x.p === undefined` guards narrow, in the fall-through, both a
// bare identifier and one property of one.
#[test]
fn nullish_equality_guard_narrows_identifier_and_property() {
    let diagnostics = check_source(
        "export function a(x: boolean | null): boolean {\n\
             if (x === null) { return false; }\n\
             return x;\n\
         }\n\
         export function b(input: { s: boolean | null }): boolean {\n\
             if (input.s === null) { return false; }\n\
             return input.s;\n\
         }\n\
         export function c(input: { s?: string }): string {\n\
             if (input.s === undefined) { return \"\"; }\n\
             return input.s;\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `a !== undefined && a <= b` narrows the right operand of the `&&` too, not
// only the guarded branch — including through a chain of guards and on one
// property of an identifier (the `ky` retry-timing shape).
#[test]
fn nullish_equality_guard_narrows_the_and_operand() {
    let diagnostics = check_source(
        "declare const make: (n: number) => number | undefined;\n\
         export function a(limit: number): number | undefined {\n\
             let result: number | undefined;\n\
             for (const year of [1, 2, 3]) {\n\
                 const candidate = make(year);\n\
                 if (candidate !== undefined && candidate <= limit) { result = candidate; }\n\
             }\n\
             return result;\n\
         }\n\
         export function b(limit: number | undefined): number {\n\
             const candidate = make(1);\n\
             if (candidate !== undefined && limit !== undefined && candidate <= limit) {\n\
                 return candidate;\n\
             }\n\
             return 0;\n\
         }\n\
         export function c(input: { s?: number }, limit: number): boolean {\n\
             return input.s !== undefined && input.s <= limit;\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    // The guard is what makes it clean; without it the comparison still reports.
    let diagnostics = check_source(
        "declare const make: (n: number) => number | undefined;\n\
         export function d(limit: number): boolean {\n\
             const candidate = make(1);\n\
             return candidate <= limit;\n\
         }\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS18048"]);
}

// `+` concatenates a string with a `bigint` (and with a `number | bigint`, the
// zod issue-`minimum` shape) exactly as tsc does, while the arithmetic
// `number + bigint` stays an error — bigint is deliberately concatenation-only.
#[test]
fn string_concatenation_accepts_bigint_operands() {
    let diagnostics = check_source(
        "declare const mixed: number | bigint;\n\
         declare const big: bigint;\n\
         export const a: string = \"Min: \" + mixed;\n\
         export const b: string = \"Min: \" + big;\n\
         export const c: string = big + \" units\";\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check_source(
        "declare const mixed: number | bigint;\n\
         declare const n: number;\n\
         export const a = n + mixed;\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2365"]);
}

// Without `exactOptionalPropertyTypes` an optional target property accepts an
// explicit `undefined`, so a required `T | undefined` source property satisfies
// it. The reverse (a genuinely wrong type) must still report.
#[test]
fn optional_target_property_accepts_undefined_source() {
    let diagnostics = check_source(
        "type Row = { sheetName?: string; n: number };\n\
         declare const src: { sheetName: string | undefined; n: number };\n\
         export const ok: Row = src;\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check_source(
        "type Row = { sheetName?: string; n: number };\n\
         declare const src: { sheetName: number | undefined; n: number };\n\
         export const bad: Row = src;\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// `(...args: infer P)` captures the whole parameter tuple, not the parameter at
// that position — what makes `ConstructorParameters<T>[0]` yield a constructor's
// first argument. The resulting tuple-typed rest parameter is then a positional
// parameter list at both call sites and in assignability.
#[test]
fn rest_infer_captures_the_parameter_tuple() {
    let diagnostics = check_source(
        "type FirstCtorArg<T extends new (..._a: any) => any> = ConstructorParameters<T>[0];\n\
         interface Ctor { new (cmd: [key: string, unix: number], opts?: number): object }\n\
         declare const call: (...args: FirstCtorArg<Ctor>) => void;\n\
         call(\"a\", 1);\n\
         export const plain: (key: string, unix: number) => void = call;\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check_source(
        "type FirstCtorArg<T extends new (..._a: any) => any> = ConstructorParameters<T>[0];\n\
         interface Ctor { new (cmd: [key: string, unix: number], opts?: number): object }\n\
         declare const call: (...args: FirstCtorArg<Ctor>) => void;\n\
         call(1, 1);\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

fn umd_program(extra: &[(&str, &str)]) -> Vec<surge_ts_diagnostics::Diagnostic> {
    let mut files: Vec<(&str, &str)> = vec![(
        "legacy.d.ts",
        "export declare function greet(name: string): string;\nexport as namespace Legacy;\n",
    )];
    files.extend_from_slice(extra);
    program(&files)
}

#[test]
fn umd_global_read_from_a_module_reports_ts2686() {
    let diagnostics = umd_program(&[("consumer.ts", "export const greeting = Legacy.greet(\"x\");\n")]);

    assert_eq!(codes(&diagnostics), vec!["TS2686"]);
    assert_eq!(file_names(&diagnostics), vec!["consumer.ts"]);
}

#[test]
fn umd_global_reports_once_per_reference_not_per_file() {
    let diagnostics = umd_program(&[(
        "consumer.ts",
        "export const a = Legacy.greet(\"x\");\nexport const b = Legacy;\n",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2686", "TS2686"]);
}

#[test]
fn umd_global_type_query_reports_ts2686_without_a_missing_name() {
    let diagnostics = umd_program(&[("consumer.ts", "export type Q = typeof Legacy;\n")]);

    assert_eq!(codes(&diagnostics), vec!["TS2686"]);
}

#[test]
fn umd_global_read_from_a_script_is_allowed() {
    // No import or export, so the file is a script and the UMD global is in
    // scope for it. Whether surge can resolve the name is a separate question —
    // this pins only that the module-only diagnostic stays off.
    let diagnostics = umd_program(&[("script.ts", "const greeting = Legacy.greet(\"x\");\n")]);

    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS2686"),
        "{:?}",
        codes(&diagnostics)
    );
}

#[test]
fn umd_global_shadowed_by_a_local_declaration_is_not_reported() {
    let diagnostics = umd_program(&[(
        "consumer.ts",
        "const Legacy = { greet: (name: string) => name };\nexport const greeting = Legacy.greet(\"x\");\n",
    )]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn umd_global_shadowed_by_a_type_only_default_import_is_not_reported() {
    // tsc reports using such a binding as a value as TS1361, never as a UMD
    // reference, so the name must read as bound here.
    let diagnostics = umd_program(&[
        ("other.d.ts", "declare const value: number;\nexport default value;\n"),
        (
            "consumer.ts",
            "import type Legacy from \"./other\";\nexport type Q = Legacy;\n",
        ),
    ]);

    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS2686"),
        "{:?}",
        codes(&diagnostics)
    );
}

#[test]
fn umd_global_is_not_reported_under_allow_umd_global_access() {
    let mut options = CheckerOptions::default();
    options.allow_umd_global_access = true;

    let diagnostics = program_with_options(
        &[
            (
                "legacy.d.ts",
                "export declare function greet(name: string): string;\nexport as namespace Legacy;\n",
            ),
            ("consumer.ts", "export const greeting = Legacy.greet(\"x\");\n"),
        ],
        options,
    );

    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS2686"),
        "{:?}",
        codes(&diagnostics)
    );
}

#[test]
fn operator_operands_are_checked_non_null_like_tsc() {
    let source = concat!(
        "declare let u: number | undefined;\n",
        "declare let s: string;\n",
        "export const a = null * 1;\n",
        "export const b = 1 - undefined;\n",
        "export const c = null + 1;\n",
        "export const d = null < 1;\n",
        "export const e = -null;\n",
        "export const f = \"k\" in null;\n",
        "export const g = u * 1;\n",
        "export const h = u + 1;\n",
        "export const i = u in {};\n",
        "export const j = null + s;\n",
        "export const k = null == 1;\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    let messages: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
        .collect();
    assert_eq!(
        messages,
        vec![
            "TS18050 The value 'null' cannot be used here.",
            "TS18050 The value 'undefined' cannot be used here.",
            "TS18050 The value 'null' cannot be used here.",
            "TS18050 The value 'null' cannot be used here.",
            "TS18050 The value 'null' cannot be used here.",
            "TS18050 The value 'null' cannot be used here.",
            "TS18048 'u' is possibly 'undefined'.",
            "TS18048 'u' is possibly 'undefined'.",
            "TS18048 'u' is possibly 'undefined'.",
        ],
    );
}

fn code_lines(source: &str, diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let start = diagnostic.span.as_ref().map_or(0, |span| span.start);
            let line = source[..start].matches('\n').count() + 1;
            let column = start - source[..start].rfind('\n').map_or(0, |index| index + 1) + 1;
            format!("{line}:{column} {}", diagnostic.code)
        })
        .collect()
}

#[test]
fn parenthesized_operand_is_unnamed_and_anchored_at_the_parentheses() {
    let source = concat!(
        "declare let u: number | undefined;\n",
        "declare let f: (() => void) | undefined;\n",
        "export const a = (null) * 2;\n",
        "export const b = (undefined) * 2;\n",
        "export const c = ((u)) * 2;\n",
        "export const d = (null).x;\n",
        "export const e = (u).toFixed();\n",
        "(null)();\n",
        "(f)();\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(
        code_lines(source, &diagnostics),
        vec![
            "3:18 TS2531",
            "4:18 TS2532",
            "5:18 TS2532",
            "6:18 TS2531",
            "7:18 TS2532",
            "8:1 TS2721",
            "9:1 TS2722",
        ],
    );
}

#[test]
fn invoking_null_or_undefined_uses_the_invocation_wording() {
    let source = concat!(
        "declare let f: (() => void) | undefined;\n",
        "null();\n",
        "undefined();\n",
        "f();\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(codes(&diagnostics), vec!["TS2721", "TS2722", "TS2722"]);
}

#[test]
fn for_in_right_operand_must_be_an_object_after_removing_nullish() {
    let source = concat!(
        "enum E { A }\n",
        "declare let u: number | undefined;\n",
        "declare let o: object | undefined;\n",
        "declare let s: string;\n",
        "declare let un: unknown;\n",
        "for (const k in null) {}\n",
        "for (const k in undefined) {}\n",
        "for (const k in u) {}\n",
        "for (const k in s) {}\n",
        "for (const k in un) {}\n",
        "for (const k in o) {}\n",
        "for (const k in E) {}\n",
        "for (const k in [1]) {}\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    let messages: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
        .collect();
    let rule = "The right-hand side of a 'for...in' statement must be of type 'any', an object type or a type parameter, but here has type";
    assert_eq!(
        messages,
        vec![
            format!("TS2407 {rule} 'never'."),
            format!("TS2407 {rule} 'never'."),
            format!("TS2407 {rule} 'number'."),
            format!("TS2407 {rule} 'string'."),
            format!("TS2407 {rule} 'unknown'."),
        ],
    );
}

#[test]
fn plus_decides_its_result_kind_before_reporting() {
    let source = concat!(
        "declare let su: string | undefined;\n",
        "declare let o: object;\n",
        "declare let an: any;\n",
        "declare let big: bigint;\n",
        "export const a: string = su + \"x\";\n",
        "export const b: string = o + \"x\";\n",
        "export const c: string = an + \"x\";\n",
        "export const d: string = an * 2;\n",
        "export const e = big + 1;\n",
        "export const f = true + 1;\n",
    );
    let diagnostics = program(&[("a.ts", source)]);
    let messages: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
        .collect();
    assert_eq!(
        messages,
        vec![
            "TS2322 Type 'number' is not assignable to type 'string'.",
            "TS2365 Operator '+' cannot be applied to types 'bigint' and '1'.",
            "TS2365 Operator '+' cannot be applied to types 'boolean' and 'number'.",
        ],
    );
}

#[test]
fn new_on_a_call_signature_is_any() {
    let source = concat!(
        "declare let f: (() => void) | undefined;\n",
        "declare let g: () => number;\n",
        "export const a = new f();\n",
        "export const b = new g();\n",
    );
    // TS18048 on the possibly-`undefined` target; without `noImplicitAny` a
    // non-`void` call signature is TS2350 rather than TS7009.
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(codes(&diagnostics), vec!["TS18048", "TS2350"]);

    let mut options = CheckerOptions::default();
    options.no_implicit_any = true;
    let diagnostics = program_with_options(&[("a.ts", source)], options);
    assert_eq!(codes(&diagnostics), vec!["TS18048", "TS7009", "TS7009"]);
}

#[test]
fn empty_array_local_evolves_with_its_mutations() {
    let source = concat!(
        "declare const names: string[];\n",
        "export function f() {\n",
        "  const out = [];\n",
        "  const early = out.slice();\n",
        "  for (const name of names) {\n",
        "    const last = out[0];\n",
        "    out.push(name);\n",
        "  }\n",
        "  const check: number = out;\n",
        "  return () => out;\n",
        "}\n",
    );
    let mut options = CheckerOptions::default();
    options.no_implicit_any = true;
    let diagnostics = program_with_options(&[("a.ts", source)], options);
    assert_eq!(
        code_lines(source, &diagnostics),
        vec!["3:9 TS7034", "4:17 TS7005", "9:9 TS2322", "10:16 TS7005"],
    );

    // Without `noImplicitAny` the literal is plainly `never[]`.
    let source = "export function f() { const out = []; out.push(1); }\n";
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

#[test]
fn without_strict_null_checks_undefined_is_in_every_type() {
    let source = concat!(
        "declare let maybe: number | undefined;\n",
        "declare let opaque: unknown;\n",
        "export const a: number = maybe;\n",
        "export const b = maybe.toFixed();\n",
        "export const c: string = undefined;\n",
        "let d = undefined;\n",
        "d = 1;\n",
        "export const e = opaque.name;\n",
        "export const f = opaque();\n",
    );
    let options = CheckerOptions {
        strict_null_checks: false,
        ..CheckerOptions::default()
    };
    let diagnostics = program_with_options(&[("a.ts", source)], options);
    assert_eq!(code_lines(source, &diagnostics), vec!["8:25 TS2339", "9:18 TS2349"]);

    // The same program under `strictNullChecks`.
    let diagnostics = program(&[("a.ts", source)]);
    assert_eq!(
        codes(&diagnostics),
        vec!["TS2322", "TS18048", "TS2322", "TS2322", "TS18046", "TS18046"],
    );
}
