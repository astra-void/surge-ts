// `x === null` / `x.p === undefined` guards narrow, in the fall-through, both a
// bare identifier and one property of one.
export function a(x: boolean | null): boolean {
if (x === null) { return false; }
return x;
}
export function b(input: { s: boolean | null }): boolean {
if (input.s === null) { return false; }
return input.s;
}
export function c(input: { s?: string }): string {
if (input.s === undefined) { return ""; }
return input.s;
}
