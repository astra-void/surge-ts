// @noImplicitReturns: true
// No `default`: the discriminant may match no clause and fall through, so
// tsc reports TS7030.
export function s(x: number) { switch (x) { case 1: return 'a'; case 2: return 'b'; } }
