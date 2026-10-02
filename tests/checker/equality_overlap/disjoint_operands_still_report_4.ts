// Provably disjoint primitive kinds keep reporting, including across a callable
// operand (the shape the whitelist inversion must not swallow).
declare const b: boolean;
export const a = b === 3;
