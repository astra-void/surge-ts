// @noUnusedParameters: true
// The parameter is read only inside a nested function declaration — the oxc
// read-walk must see it, so no TS6133.
export function f(p: number): void { function inner(): number { return p; } inner(); }
