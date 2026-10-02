// @noImplicitReturns: true
// `while (true)` with no `break` never falls through, so the end is
// unreachable and tsc emits no TS7030. (surge's separate always-truthy note
// for `while (true)` is out of scope here, so assert only TS7030's absence.)
export function g(x: number) { while (true) { if (x > 0) return 1; } }
