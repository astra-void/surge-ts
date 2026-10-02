// @noImplicitReturns: true
// Annotated return type still routes through TS2366, independent of the flag.
export function e(x: number): number { if (x > 0) return 1; }
