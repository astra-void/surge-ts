// A named alias to a union narrows even when it is the whole declared type,
// with no enclosing union node.
type O1 = { in: string };
type O2 = { map: string };
type Field = O1 | O2;
function f(v: Field): string {
if ("map" in v) {
return v.map;
}
return v.in;
}
