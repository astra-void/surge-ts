# relational-comparable-relation-basic

tsc checks `<`, `>`, `<=` and `>=` between two non-numeric operands with
`areTypesComparable` — the comparable relation, in either direction
(`checkBinaryLikeExpression`, checker.go). surge asked assignability instead,
which is stricter wherever the comparable relation differs from it
(relater.go): an optional member is read with `undefined`, so two optional
members always overlap and an optional source is not held to a required
target; the simple type rules also hold with source and target swapped
(`string` against `"x"`, `object` against a shaped type); a tuple relates to an
array when some element does; and single signatures compare with their type
parameters erased. `never` is numeric (`never < "a"` is still an error).

The second block pins the pairs that stay errors in tsc.
