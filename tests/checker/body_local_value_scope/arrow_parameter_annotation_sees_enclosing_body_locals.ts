export function outer() {
const s = { a: 1 };
const g = (x: typeof s) => x.a;
return g(s);
}
