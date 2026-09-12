# element-access-typeof-narrowing-basic

`typeof args[0] === 'string'` narrowed nothing. The identifier path narrows a
binding's own symbol, which an element access has none of — an array has one
element type for every index — so the tag test never reached the per-access
record that the truthiness and nullish guards already write. ts-pattern's
`select()` reads `typeof args[0] === 'string' ? args[0] : undefined` over a rest
tuple.

Two halves, and both are needed:

- The guard collector now recognises a `typeof` test on an element access, so
  the statement form (`inAStatement`) narrows.
- The expected-type ternary applies the same per-access narrowing, so the
  expression form (`inATernary`) does too. That path is separate from the
  statement one and was the reason the `if` worked while the `?:` did not.

`theUnknownKeywordTakesTheTag` pins the third piece. A union member written as
the `unknown` keyword carries no typeof tag, so it survived the filter and left
the result unassignable to what the guard had just proved. In the matching
branch the tag *is* the type, which is how tsc reads it. Only the genuine
keyword qualifies — `Type::Unknown` is the degradation sentinel, and rewriting
that would claim knowledge surge does not have.

`theNarrowedReadIsStillTheTag` is the intentional error: the true branch really
is `string`, so reading it as a `number` reports.
