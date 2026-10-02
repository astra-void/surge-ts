export function f() {
type P<X> = { v: X };
const p: P<number> = { v: 1 };
return p.v;
}
