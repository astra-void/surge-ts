// A body-local value goes out of scope with its body; a sibling body must not
// see it.
export function a() {
const s = { v: 1 };
type S = typeof s;
const x: S = s;
return x;
}
export function b() {
type S2 = typeof s;
const y: S2 = null as any;
return y;
}
