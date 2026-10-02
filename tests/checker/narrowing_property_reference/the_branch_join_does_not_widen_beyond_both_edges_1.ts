// The join is a union of both edges, so an assignment that keeps the nullish
// member (or a condition that proves nothing) still reports.
declare function maybe(): number | undefined;
export function f(x?: number): number {
if (!x) {
x = maybe();
}
return x;
}
