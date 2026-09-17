# increment-operand-write-rules-basic

`x++` was dropped by the parser entirely — `UpdateExpression` had no arm — so
the operand was never even walked: neither the write it performs nor its own
errors were checked.

The four forms (prefix/postfix, increment/decrement) now lower to one
`Update` expression, which tsc checks identically in
`checkPre/PostfixUnaryExpression`. The order the rules run in is what tsc's
output shape depends on: `checkExpression` reports the rejected write first
(TS2588 for a `const` binding, TS2540 for a `readonly` member) and hands back
the error type, which then satisfies the arithmetic rule — so a `const string`
operand reports only TS2588, never both. `checkNonNullType` runs next, which is
why `number | undefined` is reported as possibly-undefined rather than as a bad
arithmetic operand. Only then does TS2356 apply.

The result type follows `getUnaryResultType`: a `bigint` operand keeps `bigint`,
everything else coerces to `number`.
