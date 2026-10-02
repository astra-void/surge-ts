// @noImplicitReturns: true
// Every path exits: the `try` returns, the `catch` throws. The construct
// never falls through, so tsc emits no TS7030 (regression: surge's Try flow
// summary used to hardcode `guarantees_exit = false`).
export function h(x: number) { try { return x; } catch (e) { throw e; } }
