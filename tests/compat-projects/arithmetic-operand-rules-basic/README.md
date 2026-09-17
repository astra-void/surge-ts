# arithmetic-operand-rules-basic

The arithmetic and bitwise arm of tsc's `checkBinaryLikeExpression`:

- two boolean operands of `&`, `|` or `^` are TS2447, suggesting the logical
  operator, instead of an operand error;
- each operand must be `any`, number-like or **bigint-like** (TS2362/TS2363) —
  surge accepted only numbers, so `big * big` was a false positive;
- mixing a bigint with a non-bigint, or `>>>` on bigints, is TS2365 on the whole
  expression; `any` counts as bigint-like there, so `any % bigint` is `bigint`.

A bigint literal (`2n`) was not modelled at all and is now `bigint`.
