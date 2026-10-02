// The join is a union of both edges, so an assignment that keeps the nullish
// member (or a condition that proves nothing) still reports.
declare function make(): number;
declare const cond: boolean;
export function f(x?: number): number {
if (cond) {
x = make();
}
return x;
}
