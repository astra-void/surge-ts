# angle-bracket-assertion-basic

`<T>expr` is the same assertion as `expr as T` — tsc checks both kinds through
`checkAssertion` — but the parser lowered only `TSAsExpression`, so an
angle-bracket assertion (including `<const>`) became an unmodelled expression
and every diagnostic depending on its type went silent. Both forms now lower
through one path, as an argument and as an ordinary expression.
