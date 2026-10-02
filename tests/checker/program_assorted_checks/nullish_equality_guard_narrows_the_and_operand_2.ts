// `a !== undefined && a <= b` narrows the right operand of the `&&` too, not
// only the guarded branch — including through a chain of guards and on one
// property of an identifier (the `ky` retry-timing shape).
// The guard is what makes it clean; without it the comparison still reports.
declare const make: (n: number) => number | undefined;
export function d(limit: number): boolean {
const candidate = make(1);
return candidate <= limit;
}
