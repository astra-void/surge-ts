// @noUnusedLocals: true
export function f(): number { const a = 1; const b = 2; if (a > 0) { return a; } return [b].map(x => x)[0]; }
