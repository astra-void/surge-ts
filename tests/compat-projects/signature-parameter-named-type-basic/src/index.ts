type F = (string) => void;
type G = (a, number, ...boolean) => void;
interface I { m(string): void; (number): void; new (any): I; }
type Foo = number; type H = (Foo) => void;
type J = (Missing) => void;
function fn(string) {}
const arrow = (number) => {};
type L = (undefined, object, symbol, bigint, unknown, never) => void;
type M = (Array, Promise) => void;
declare function d(string): void;
class C { m(string) {} }
type N = (a: string, string?) => void;
type Ok = (x: string, ...rest: number[]) => void;
type Ctor = new (string) => I;
export {}
