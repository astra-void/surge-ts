# property-path-impossible-union-member-basic

The leaf narrowers answer `None` for two different facts — "nothing to narrow
here" and "this union member cannot satisfy the guard at all" — and the union
walk read both as "keep the member unchanged".

The AWS SDK's union-member pattern is where that matters. Every member of
`Field` declares the other members' keys as `?: never`, so
`field.arrayValue !== undefined` leaves exactly one member possible. At the
`?: never` leaf the effective type `never | undefined` collapses to plain
`undefined`, which has no union to split, so the leaf answered `None`, the member
stayed, and every later `field.arrayValue` read was still optional — ten
`TS18048` in drizzle's `aws-data-api` reader, all in one function.

`readField` is that reader. `aMemberThatSurvivesIsStillKept` is the control for
the other direction: the `=== undefined` branch keeps the member whose property
*is* undefined, so `longValue` is readable there.

`theOtherMemberIsGoneInTheNarrowedBranch` is the intentional error and pins the
removal: with `arrayValue` present the value is `ArrayMember`, whose `longValue`
is `?: never`, so assigning it to `number` reports.
