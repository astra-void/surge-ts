# update-operand-arithmetic-first

tsc's `checkPrefixUnaryExpression`/`checkPostfixUnaryExpression` check a
`++`/`--` operand as arithmetic first (after `checkNonNullType`) and only then
as a reference: an operand that is no reference (`++1`, `count()++`) is TS2357
— TS2777 for an optional chain — when it is arithmetic, and TS2356 alone when
it is not (`++true`, `flag()--`, `--"text"`). oxc rejects such an operand as a
write target while parsing; surge had numbered that TS2357 whatever the
operand, beside the checker's TS2356.
