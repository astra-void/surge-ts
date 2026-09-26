//! `properties::child_properties` against the trees the parser builds: for
//! every attached node, the properties of its kind reach exactly the node's
//! children, visiting them in property order walks the source in order, and
//! the sources below give every property a value somewhere.

use std::collections::HashSet;

use surge_ts_tsc_syntax::properties::{Slot, child_properties, resolve};
use surge_ts_tsc_syntax::{Kind, ParseOptions, SyntaxTree};

/// Sources the parser accepts. Some break rules only the checker enforces,
/// to reach the properties only such code fills.
const SOURCES: &[(&str, &str)] = &[
    (
        "modules.ts",
        r#"
import def, { a as b, type T as U, c, "str" as str } from "./m" with { type: "json" };
import * as ns from "./ns";
import type { V } from "./v";
import type TypeDefault from "./v";
import "./side-effect";
import defer * as lazy from "./lazy";
import eq = require("./eq");
import type eqType = require("./eq");
import alias = ns.inner.value;
export import reexported = ns.inner;
export { b as renamed, c, def as "string name" };
export type { V as W } from "./v";
export * from "./all";
export type * from "./all";
export * as everything from "./all" with { type: "json" };
export default function* gen<T>(this: Window, a?: T, b = 1, ...rest: T[]): Generator<T> {
    yield* rest;
    yield a!;
    yield;
}
export = def;
export as namespace Lib;
export declare const declared: number;
export type Exported = string;
"#,
    ),
    (
        "declarations.ts",
        r#"
declare module "ambient" {
    export const x: number;
}
declare module "shorthand";
declare global {
    interface Window { y: string }
}
namespace Outer.Inner.Deepest {
    export let z = 1;
}
module Legacy {}
enum Color { Red, Green = 2, "Blue" = Green << 1 }
const enum Flags { A = 1 << 0 }
abstract class Base<in out T extends object = {}> extends Mixin<T>(Object) implements I1, I2<T> {
    @decorator() static readonly #priv?: string = "x";
    declare field!: number;
    accessor acc = 1;
    static { init(); }
    ;
    constructor(private readonly p: number, @inject() public q?: string, { r } = {}, [s] = []) {
        super();
        super.method();
        super["method"]();
    }
    get value(): T { return this.v; }
    set value(v: T) {}
    static get count() { return 0; }
    static set count(n) {}
    abstract method<U>(u: U): void;
    *gen() {}
    async am?(): Promise<void> {}
    [Symbol.iterator]() {}
    [key: string]: any;
    static [n: number]: string;
    override toString() { return ""; }
}
export interface I<T> extends A, B<T> {
    (x: number): string;
    <U>(u: U): U;
    new (x: number): I<T>;
    new <U>(u: U): I<U>;
    readonly [key: string]: unknown;
    method?<U>(u: U): void;
    prop?: string;
    readonly ro: number;
    get acc(): number;
    set acc(v: number);
}
function overload(a: string): void;
function overload(a: any) {}
declare function ambient(): void;
let definite!: number;
var { a: renamedA, b = 1, ...restObj } = obj, [first, , third = 3, ...restArr] = arr;
"#,
    ),
    (
        "types.ts",
        r#"
type Primitive = string | number | bigint | boolean | symbol | null | undefined | void | never | unknown | any | object;
type Lit = "s" | 1 | -1 | 1n | true | false | `tpl` | `a${string}b${number}c`;
type Fn = <T>(a: T, b?: number, ...c: string[]) => T;
type Ctor = abstract new <T>(x: T) => T;
type Obj = {
    a: string;
    b?: number;
    readonly c: boolean;
    (x: number): void;
    new (): Obj;
    [k: string]: unknown;
    m(): void;
    get g(): number;
    set g(v: number);
};
type Arr = string[];
type Tup = [a: string, b?: number, ...rest: boolean[]];
type Tup2 = [string, number?, ...boolean[]];
type Idx = Obj["a"];
type Keys = keyof Obj;
type Uniq = unique symbol;
type RO = readonly string[];
type Mapped = { +readonly [K in keyof Obj as `get${K & string}`]-?: Obj[K] };
type Mapped2 = { readonly [K in "a"]?: K };
type Cond<T> = T extends string ? "s" : T extends number ? "n" : never;
type Inf<T> = T extends Promise<infer U> ? U : T;
type InfC<T> = T extends [infer H extends string, ...infer R] ? H : never;
type Paren = (string | number)[];
type Q = typeof import("./m");
type Imp = import("./m", { with: { "resolution-mode": "import" } }).Foo.Bar<string>;
type TQ = typeof value.prop<string>;
type Pred = (x: unknown) => x is string;
type Asserts = (x: unknown) => asserts x is string;
type AssertsOnly = (x: unknown) => asserts x;
type AssertsThis = { check(): asserts this is Foo };
type ThisPred = { isFoo(): this is Foo };
type Qualified = A.B.C<D>;
type Intersect = A & B & (C | D);
type Leading = | A | B;
type LeadingAmp = & A & B;
type Optional = [string?];
type Generic<const T, in U, out V = T> = T;
declare function thisParameter(this: Foo): this is Bar;
declare let jsdoc1: ?string;
declare let jsdoc2: string?;
declare let jsdoc3: !string;
declare let jsdoc4: string!;
declare let jsdoc5: *;
"#,
    ),
    (
        "expressions.ts",
        r#"
const o = { a, b: 1, [c]: 2, ...d, m() {}, get g() { return 1; }, set g(v) {}, async *ag() {}, "s": 1, 2: 3 };
({ a, b: [c, , ...d], e = 1, ...f } = obj);
[x, , y = 2, ...z] = arr;
a?.b?.[c]?.(d);
a!.b![c]!;
new Foo<string>(1, ...args);
new Foo;
new Foo.Bar();
f<string>(1);
g<string>;
tag`a${b}c${d}e`;
tag<string>`x`;
a?.`x`;
<T>value;
value as T;
value satisfies T;
x ? y : z;
a + b * c ** d;
a = b += c;
a ??= b ||= c &&= d;
x++; --y; !z; ~w; -v; +u;
typeof t; void 0; delete o.p;
async function af() {
    await p;
    for await (const x of xs) {}
    await using ares = getAsyncResource();
}
const arrow = async (a: number): Promise<void> => {};
const simple = x => x;
const asyncSimple = async x => x;
const generic = <T,>(t: T) => t;
const fe = async function* named<T>(a: T): AsyncGenerator<T> {};
const ce = class Named<T> extends Base<T> implements I { x = 1; };
const dec = @decorator class {};
(a, b);
`template ${a} and ${b}`;
/regex/g;
[1, , 2, ...rest];
function meta() {
    new.target;
    import.meta.url;
    import("./m");
    this.x;
}
"#,
    ),
    (
        "statements.ts",
        r#"
label: for (let i = 0; i < 10; i++) {
    if (i) continue label;
    else break label;
}
for (;;) break;
for (const k in obj) {}
for (const v of arr) {}
for (x of y);
for (x in y);
for (var i2 = 0, j = 1; ; ) {}
while (c) {}
do {} while (c);
switch (x) {
    case 1:
    case 2:
        f();
        break;
    default:
        g();
}
try { a(); } catch (e) { b(); } finally { c(); }
try {} catch {}
throw new Error();
if (a) b(); else if (c) d(); else e();
with (obj) {}
debugger;
;
{ nested; }
function f() { return 1; }
function g() { return; }
using res = getResource();
"#,
    ),
    (
        "grammar.ts",
        r#"
function tp<T extends +1>() {}
({ async a: 1, b?: 2, c!: 3, async d, e?, f!, m!() {} });
class K {
    public constructor<T>(): void {}
    get g<T>(x) { return 1; }
    set s<T>(v): void {}
    public static {}
    @dec static {}
    m?() {}
}
interface J {
    readonly m(): void;
    p: number = 1;
}
type M = { [K in T]: X; extra: 1 };
@dec export as namespace NS;
@dec export default x;
@dec export { y };
export import z from "z";
"#,
    ),
    (
        "jsx.tsx",
        r#"
const el = <div className="a" {...props} data-x={1} ns:attr="v" disabled>
    text {expr} {/* comment */}
    <Self.Closing<string> prop={<b />} />
    <Generic<number>>child</Generic>
    <>fragment {...spread}</>
    <a:b />
    <this.member />
    {cond ? <i /> : null}
</div>;
const generic = <T,>(t: T) => t;
"#,
    ),
    (
        "ambient.d.ts",
        r#"
declare namespace NS {
    function f(): void;
    const x: number;
    class C { private constructor(); m(): void; }
}
export {};
"#,
    ),
];

/// Sources the parser recovers from.
const RECOVERED: &[(&str, &str)] = &[
    ("missing-declaration.ts", "@dec;\nx = @dec;\nclass C { @dec }\n"),
    ("super.ts", "class S extends B { m() { super; super<T>(); } }\n"),
    ("jsx-recovery.tsx", "const unclosed = <div><span></div>;\nconst siblings = <a></a><b></b>;\n"),
];

/// Kinds with properties no source can fill: only the JSDoc type parser,
/// which this port leaves out, builds them.
const UNBUILT: &[Kind] = &[Kind::JSDocOptionalType, Kind::JSDocVariadicType];

fn parse(file_name: &str, text: &str) -> SyntaxTree {
    SyntaxTree::parse(text, &ParseOptions::for_file_name(file_name).expect("a script file name"))
}

/// Checks every attached node of the tree, and records the properties that
/// have a value.
fn check_tree(file_name: &str, text: &str, filled: &mut HashSet<(Kind, &'static str)>) {
    let tree = parse(file_name, text);
    for id in 0..tree.node_count() as u32 {
        if !tree.is_attached(id) {
            continue;
        }
        let kind = tree.kind(id);
        let at = tree.start(text, id);
        let mut visited = Vec::new();
        for property in child_properties(kind) {
            let nodes = resolve(&tree, id, property.slot).nodes().to_vec();
            if !nodes.is_empty() {
                filled.insert((kind, property.name));
            }
            visited.extend(nodes);
        }
        let mut reached = visited.clone();
        reached.sort_unstable();
        let mut children = tree.children(id);
        children.sort_unstable();
        assert_eq!(
            reached, children,
            "{file_name}: the properties of the {kind:?} at {at} reach {visited:?}, its children are {children:?}"
        );
        for pair in visited.windows(2) {
            let (before, after) = (pair[0], pair[1]);
            assert!(
                tree.end(before) <= tree.pos(after),
                "{file_name}: the {kind:?} at {at} visits the {:?} at {} after the {:?} ending at {}",
                tree.kind(after),
                tree.pos(after),
                tree.kind(before),
                tree.end(before),
            );
        }
    }
}

#[test]
fn properties_reach_each_child_once_in_source_order() {
    let mut filled = HashSet::new();
    for &(file_name, text) in SOURCES {
        let diagnostics = parse(file_name, text).syntactic_diagnostics();
        assert!(diagnostics.is_empty(), "{file_name} has syntax errors: {diagnostics:?}");
        check_tree(file_name, text, &mut filled);
    }
    for &(file_name, text) in RECOVERED {
        assert!(!parse(file_name, text).syntactic_diagnostics().is_empty(), "{file_name} has no syntax error");
        check_tree(file_name, text, &mut filled);
    }
}

#[test]
fn every_property_is_filled_by_some_source() {
    let mut filled = HashSet::new();
    for &(file_name, text) in SOURCES.iter().chain(RECOVERED) {
        check_tree(file_name, text, &mut filled);
    }
    let filled = &filled;
    let unfilled: Vec<String> = Kind::ALL
        .iter()
        .filter(|kind| !UNBUILT.contains(kind))
        .flat_map(|&kind| {
            child_properties(kind)
                .iter()
                .filter(move |property| !filled.contains(&(kind, property.name)))
                .map(move |property| format!("{kind:?}.{}", property.name))
        })
        .collect();
    assert!(unfilled.is_empty(), "no source fills {unfilled:?}");
}

#[test]
fn properties_have_distinct_names_and_slots() {
    for &kind in Kind::ALL.iter() {
        let properties = child_properties(kind);
        for (index, property) in properties.iter().enumerate() {
            for earlier in &properties[..index] {
                assert_ne!(earlier.name, property.name, "{kind:?} names two properties {}", property.name);
                assert_ne!(earlier.slot, property.slot, "{kind:?} reads {:?} twice", property.slot);
            }
            let list_slot = matches!(
                property.slot,
                Slot::Modifiers | Slot::TypeParameters | Slot::TypeArguments | Slot::Parameters | Slot::List(_)
            );
            assert_eq!(property.list, list_slot, "{kind:?}.{}", property.name);
        }
    }
}

#[test]
fn postfix_tokens_split_by_kind() {
    let text = "class C { a?: number; b!: number; }\n({ m!() {}, n?() {} });\n";
    let tree = parse("postfix.ts", text);
    let token = |name: &str, property: &str| {
        let member = (0..tree.node_count() as u32)
            .find(|&id| {
                matches!(tree.kind(id), Kind::PropertyDeclaration | Kind::MethodDeclaration)
                    && tree.node(id).name.is_some_and(|name_id| tree.text(name_id) == name)
            })
            .expect("the member is in the tree");
        let slot = child_properties(tree.kind(member))
            .iter()
            .find(|candidate| candidate.name == property)
            .expect("the kind has the property")
            .slot;
        resolve(&tree, member, slot).nodes().first().map(|&id| tree.kind(id))
    };
    assert_eq!(token("a", "questionToken"), Some(Kind::QuestionToken));
    assert_eq!(token("a", "exclamationToken"), None);
    assert_eq!(token("b", "questionToken"), None);
    assert_eq!(token("b", "exclamationToken"), Some(Kind::ExclamationToken));
    assert_eq!(token("m", "exclamationToken"), Some(Kind::ExclamationToken));
    assert_eq!(token("n", "questionToken"), Some(Kind::QuestionToken));
}
