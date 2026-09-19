export {};
class Base { public foo: string = ""; }
class E { private foo: string = ""; }
class E2 { private foo: string = ""; }
class P { protected foo: string = ""; }
class P2 extends P {}
declare let base: Base; declare let e: E; declare let e2: E2; declare let p: P; declare let p2: P2;
declare let o: { foo: string };
base = e;
e = base;
e = e2;
e = e;
o = e;
o = p;
p = o;
p = p2;
class D extends E {}
declare let d: D;
e = d;
