# inferred-type-predicates-basic

tsc infers a type predicate for a function with no return annotation whose
body returns one boolean expression (`getTypePredicateFromBody`,
`checkIfExpressionRefinesAnyParameter`, checker.go): the first parameter the
expression narrows to some `T` when it holds, and of which nothing is left
when it fails *starting from `T`*, becomes `x is T`. The parameter is typed as
the flow leaves it at the `return`, so a preceding `if (…) throw` narrows it
first; a parameter the body assigns, a `boolean` one and a non-boolean
returned expression infer nothing. surge required the two branches to
partition the declared type, which a non-union parameter (`x: object` tested
`x instanceof Date`) and an `unknown` one never do, skipped every body with a
statement other than an expression before the `return`, and did not read
through `satisfies`. The `false` branch of a predicate over a subject it
already covers is `never` (`getNarrowedType`), and an `in` test rules `null`
out when the key is present.

Every error here is also tsc's.
