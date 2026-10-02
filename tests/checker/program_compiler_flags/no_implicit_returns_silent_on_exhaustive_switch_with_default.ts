// @noImplicitReturns: true
// Every clause returns and a `default` makes the switch exhaustive, so no
// path falls through — tsc emits no TS7030.
export function s(x: number) { switch (x) { case 1: return 'a'; default: return 'b'; } }
