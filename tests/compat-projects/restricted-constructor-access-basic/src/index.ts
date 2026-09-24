export {};
class Base { protected x = 1; protected static sx = 1; }
class Derived extends Base {
  f(b: Base, d: Derived, s: Sibling) { b.x; d.x; this.x; s.x; Base.sx; Derived.sx; }
  g(b: Base) { b.x = 2; }
}
class Sibling extends Base {}
class Priv { private constructor() {} static make() { return new Priv(); } }
new Priv();
class Prot { protected constructor() {} }
class Sub extends Prot { static make() { return new Prot(); } }
new Prot();
class Sub2 extends Priv {}
class Sub3 extends Prot {}
function g<T extends Base>(t: T) { t["x"]; }
function h(t: Base) { t["x"]; }
class Outer { private constructor() {} static Inner = class { make() { return new Outer(); } }; }
class PB { private x = 1; protected y = 2; z = 3; }
type Q1<T extends PB> = T["x"];
type Q2<T extends PB> = T["y"];
type Q3<T extends PB> = T["z"];
function q<T extends PB>(t: T): T["x"] { return t["x"]; }
