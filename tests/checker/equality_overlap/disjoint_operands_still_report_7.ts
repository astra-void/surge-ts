// Provably disjoint primitive kinds keep reporting, including across a callable
// operand (the shape the whitelist inversion must not swallow).
declare const arr: string[];
declare const n: number;
export const a = arr === n;
