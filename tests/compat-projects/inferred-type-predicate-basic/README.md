# inferred-type-predicate-basic

`getTypePredicateFromBody` (TS 5.5): a callback with no return annotation
whose body only narrows its own parameter *is* a type predicate, so
`nums.filter(x => x !== null)` is `number[]`. The narrowing has to
partition the parameter's type — the false branch must be exactly what
the true branch leaves out — so truthiness (`!!x`, where `0` is falsy)
and a narrowing mixed with an unrelated test infer nothing.
