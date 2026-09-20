declare function assertFn(expression: any): asserts expression;
declare function isDate(v: unknown): v is Date;
declare function assertDate(v: unknown): asserts v is Date;
export function a(u: unknown) {
  if (u instanceof Date) {
    const n: number = u;
    return n;
  }
  return 0;
}
export function b(u: unknown) {
  assertFn(u instanceof Date);
  const n: number = u;
  return n;
}
export function c(u: unknown) {
  if (!(u instanceof Date)) { return 0; }
  const n: number = u;
  return n;
}
export function d(u: unknown) {
  if (isDate(u)) {
    const n: number = u;
    return n;
  }
  return 0;
}
export function e(u: unknown) {
  assertDate(u);
  const n: number = u;
  return n;
}
export function f(u: unknown) {
  if (typeof u === "string") {
    const n: number = u;
    return n;
  }
  return 0;
}
export function g(u: unknown) {
  assertFn(typeof u === "string");
  const n: number = u;
  return n;
}
export function i(u: unknown) {
  return typeof u === "number" ? u.toFixed() : u.length;
}
