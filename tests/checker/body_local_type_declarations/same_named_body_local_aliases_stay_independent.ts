// Two sibling bodies may declare the same local name with different bodies;
// the program-wide resolution caches are keyed on the declaration name, so the
// two must not collapse onto one entry.
export function a() {
type Q = { p: string };
const v: Q = { p: 1 };
return v;
}
export function b() {
type Q = { p: number };
const v: Q = { p: 1 };
return v;
}
