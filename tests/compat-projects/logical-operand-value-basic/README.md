# logical-operand-value-basic

`a && b` is the definitely-falsy part of `a` joined with `b`
(`extractDefinitelyFalsyTypes`), and `a || b` is what is left of `a` once
truthy joined with `b` (`removeDefinitelyFalsyTypes` under
`getNonNullableType`). surge kept only the nullish handling for `||`, so a
`boolean` left operand stayed `boolean` instead of `true`, and its falsy part
dropped `null` and `bigint`.
