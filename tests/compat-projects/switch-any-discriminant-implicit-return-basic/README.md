# switch-any-discriminant-implicit-return-basic

A `default`-less switch ends its function only when it is exhaustive, and tsc's
`computeExhaustiveSwitchStatement` calls a switch exhaustive only over a
discriminant of a literal type (`isLiteralType`) whose members the cases all
name. An `any` discriminant is never covered, and a `typeof` switch over a top
type needs every tag, so under `noImplicitReturns` both report TS7030. surge
treated an `any` discriminant as covered.
