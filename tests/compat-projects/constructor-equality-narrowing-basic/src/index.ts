class C1 { property1!: string }
class C2 { value!: number }
class Sub extends C1 { extra!: boolean }
export function f(a: C1 | number, b: C1 | C2, c: C1 | Sub, d: { inner: C1 | C2 }, e: string | C1) {
  if (a.constructor == C1) { a.property1; } else { a.property1; }
  if (a["constructor"] === C1) { a.property1; }
  if (C2 === b.constructor) { b.value; }
  if (b.constructor !== C1) { b.property1; } else { b.property1; }
  if (c.constructor === C1) { c.extra; }
  if (d.inner.constructor === C2) { d.inner.value; }
  if (e.constructor === String) { e.length; }
  const viaTernary = a.constructor === Number ? a.toFixed() : 0;
  const viaAnd = b.constructor === C1 && b.property1;
  return [viaTernary, viaAnd];
}
