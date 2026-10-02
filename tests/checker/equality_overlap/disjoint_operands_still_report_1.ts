// Provably disjoint primitive kinds keep reporting, including across a callable
// operand (the shape the whitelist inversion must not swallow).
declare const n: number;
export const a = n === "x";
