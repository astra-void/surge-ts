# narrowing-logical-operand-edges

tsc's binder gives each branch of a `&&`/`||` condition one flow edge per
way the condition can come out: the false branch of `A && B` is reached when
`A` fails or when `A` holds and `B` then fails, and the true branch of
`A || B` when `A` holds or when `A` fails and `B` then holds. The reference
is the union of what those edges narrow it to. surge's expression and module
paths had no false branch for `&&` at all, so the `else` of
`typeof x !== "string" && typeof x !== "number"` (tsc's
`typeGuardOfFormExpr1AndExpr2`) kept `x` whole, and both paths narrowed the
second operand from the declared type instead of from what the first left.

`script.ts` is at the top level of a script; `bodies.ts` runs the same forms
in function bodies and in `?:` conditions. The `bad` declaration is the one
intended error.
