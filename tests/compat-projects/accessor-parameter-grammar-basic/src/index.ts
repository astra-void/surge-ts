class C {
  get a(x: number) { return x; }
  get b(this: C, x: number) { return x; }
  set d(a: number, b: number) {}
  set e(...a: number[]) {}
  set f(a?: number) {}
  set g(a: number = 1) {}
  set h(this: C, a: number) {}
  get i(this: C) { return 1; }
  set j(v: number) { return 1; }
  set k(v: number) { return; }
  get ok() { return 1; }
  set ok(v: number) {}
}
const o = {
  get a(x: number) { return x; },
  set e(...a: number[]) {},
  set f(a?: number) {},
  set g(a: number = 1) {},
  set j(v: number) { return 2; },
  get fine() { return 1; },
};
interface I {
  get a(x: number): number;
  set d(a: number, b: number);
  set e(...a: number[]);
  set f(a?: number);
  get x(this: I): number;
  get fine(): number;
}
export {};
