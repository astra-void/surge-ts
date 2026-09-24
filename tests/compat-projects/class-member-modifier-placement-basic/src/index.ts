class A {
  accessor m() {}
  accessor get x() { return 1; }
}
class B { constructor(accessor x: number) {} }
abstract class D {
  abstract y = 1;
  abstract accessor z = 2;
  abstract w: number;
  abstract constructor();
}
class E { constructor(abstract x: number) {} }
abstract enum En {}
abstract interface I {}
abstract namespace N {}
abstract const c = 1;
class F {
  "constructor" = 1;
}
class F2 {
  ["constructor"] = 1;
}
function pattern({ a }?: { a: number }) {}
const arrow = ({ a }?: { a: number }) => a;
class K { m([b]?: [number]) {} }
declare function sig({ a }?: { a: number }): void;
function defaulted({ a }: { a: number } = { a: 1 }) {}
export {};
