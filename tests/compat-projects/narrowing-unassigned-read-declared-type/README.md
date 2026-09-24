# narrowing-unassigned-read-declared-type

tsc's `checkIdentifier` starts the flow type of a variable it cannot assume
initialized at its declared type plus `undefined`. A read whose flow type
still admits `undefined` there is reported as used before being assigned
(TS2454), and is then typed as the declared type "to reduce follow-on
errors" (checker.go) — whatever a guard narrowed the variable to. So in
`typeof x === "string" ? x.substr : x.toFixed`, with `var x: string | number;`
never assigned, `x` in the false branch is `number | undefined`, reports
TS2454, and reads as `string | number`, which has no `toFixed` (tsc's
`typeGuardRedundancy`). The true branch's `string` has no `undefined`, so it
keeps the narrowed type.

surge reports TS2454 from a definite-assignment pass that runs after the
top-level statements are type-checked, and typed such a read by the guard.
The pass now runs silently first to find those reads, and each is typed as
the binding's declared type.

A `declare var` is assumed initialized and an assigned variable is not
unassigned there, so both narrow as usual. Every error here is also tsc's.
