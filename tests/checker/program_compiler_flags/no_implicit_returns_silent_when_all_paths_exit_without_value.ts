// @noImplicitReturns: true
// Every path exits explicitly (`return 1` / bare `return;`), so there is no
// implicit fall-through — tsc emits nothing here even under noImplicitReturns.
export function c(x: number) { if (x > 0) return 1; return; }
