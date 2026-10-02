export function f() {
type T = { a: number };
const x: T = { a: "wrong" };
return x;
}
