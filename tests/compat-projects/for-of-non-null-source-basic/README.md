# for-of-non-null-source-basic

tsc's `checkRightHandSideOfForOf` checks the iterated expression with
`checkNonNullExpression`: a possibly-`undefined` source is TS18048 (TS2532
when unnamed), an `unknown` one TS18046, and `null` TS18050 — for `for await`
too. surge iterated the non-`undefined` part silently.
