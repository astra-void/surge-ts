type P = { a?: number };
declare const p: P;
export function f() {
  const { a = 0 } = p;
  const x: number = a;
  return x;
}
