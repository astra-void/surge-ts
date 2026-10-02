export function f() {
type A = { n: number };
type B = { a: A };
const b: B = { a: { n: 1 } };
return b.a.n;
}
