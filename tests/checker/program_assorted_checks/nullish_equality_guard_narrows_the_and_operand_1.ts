// `a !== undefined && a <= b` narrows the right operand of the `&&` too, not
// only the guarded branch — including through a chain of guards and on one
// property of an identifier (the `ky` retry-timing shape).
declare const make: (n: number) => number | undefined;
export function a(limit: number): number | undefined {
let result: number | undefined;
for (const year of [1, 2, 3]) {
const candidate = make(year);
if (candidate !== undefined && candidate <= limit) { result = candidate; }
}
return result;
}
export function b(limit: number | undefined): number {
const candidate = make(1);
if (candidate !== undefined && limit !== undefined && candidate <= limit) {
return candidate;
}
return 0;
}
export function c(input: { s?: number }, limit: number): boolean {
return input.s !== undefined && input.s <= limit;
}
