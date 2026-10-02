// Provably disjoint primitive kinds keep reporting, including across a callable
// operand (the shape the whitelist inversion must not swallow).
declare const s: symbol;
declare const n: number;
export const a = s === n;
