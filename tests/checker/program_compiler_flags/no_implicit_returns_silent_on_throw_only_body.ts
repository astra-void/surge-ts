// @noImplicitReturns: true
// A function that only throws (no `return <value>`) infers a `void` return
// type; tsc skips TS7030. A `throw` must not count as a value return.
export function p(x: number) { if (x > 0) { throw x; } }
