// Provably disjoint primitive kinds keep reporting, including across a callable
// operand (the shape the whitelist inversion must not swallow).
declare const g: bigint;
declare const s: string;
export const a = g === s;
