// Provably disjoint primitive kinds keep reporting, including across a callable
// operand (the shape the whitelist inversion must not swallow).
declare const f: (n: number) => void;
export const a = f === 204;
