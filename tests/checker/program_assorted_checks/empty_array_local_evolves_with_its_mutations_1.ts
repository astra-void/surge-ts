// @noImplicitAny: true
declare const names: string[];
export function f() {
  const out = [];
  const early = out.slice();
  for (const name of names) {
    const last = out[0];
    out.push(name);
  }
  const check: number = out;
  return () => out;
}
