function f(x: unknown): y is string { return true; }
type F = (x: unknown) => y is string;
const g = (x: unknown): y is string => true;
class C { m(x: unknown): y is string { return true; } }
interface I { m(x: unknown): y is string; (x: unknown): y is string; }
function a(x: unknown): asserts y { }
function b({ a }: { a: unknown }): a is string { return true; }
function c([b]: [unknown]): b is string { return true; }
function d({ c: { d } }: { c: { d: unknown } }): d is string { return true; }
class G { get x(): this is G { return true; } }
let ct: new (x: any) => x is string;
interface J { new (x: unknown): y is string; }
function ok(x: unknown): x is string { return true; }
export {};
