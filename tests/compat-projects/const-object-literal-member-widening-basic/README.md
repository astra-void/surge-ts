# const-object-literal-member-widening-basic

A property initializer is a mutable location: tsc types it with
`checkExpressionForMutableLocation`, which widens a fresh literal, so even
`const flat = { count: 1 }` is `{ count: number }` and assigning it to
`{ count: 1 }` is TS2322. surge widened object literal members only for
`let`/`var`, so under `const` the literals leaked into the binding's type.

Pinned as clean: an object literal checked against a literal contextual type,
`as const` on the whole literal, and `as const` on one member — an assertion
yields a regular literal, which tsc does not widen.
