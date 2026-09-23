export {};

// Rejected: no `return` statement at all is TS2355, and a `throw` is not one.
export function onlyThrowsInLoop(x: boolean): number { while (x) { throw new Error(); } }
export function throwsSometimes(x: boolean): number { if (x) throw new Error(); }
export function empty(): number { }
export function unreachableReturn(): number { if (true) { } else { return 1; } }
export const arrow = (x: boolean): number => { if (x) throw new Error(); };
export class Holder {
    method(x: boolean): number { while (x) { throw new Error(); } }
}

// Rejected: a `return` exists, so a reachable end is TS7030 instead.
export function returnsSometimes(x: boolean): number { if (x) return 1; }
export function throwsOrReturns(x: number): number { if (x > 1) throw new Error(); if (x > 0) return 1; }

// Accepted: the end point is not reachable.
export function alwaysThrows(): number { throw new Error(); }
export function alwaysReturns(x: boolean): number { if (x) { return 1; } else { throw new Error(); } }
