# unary-operand-rules-basic

The `+`/`-`/`~` arm of tsc's `checkPrefixUnaryExpression`: a possibly nullish
operand is reported (TS18048), a symbol operand is TS2469, and unary `+` on a
bigint is TS2736 since it can only produce a `number`. surge reported none of
them, and `~` was not typed at all (it now types like `-`: `number`, or
`bigint` for a bigint operand).
