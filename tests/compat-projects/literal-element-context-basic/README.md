# literal-element-context-basic

`isLiteralOfContextualType`: an array literal's elements stay literals
when their contextual type is a type variable constrained to that
primitive (`T extends string` infers `"a" | "b"` from `["a", "b"]`), or
when the contextual return type instantiates it to a literal union. An
unconstrained `T` with no such context infers `string`. surge widened the
elements before inference in every case, so a constrained `T` came back
as `string` and the result fit nothing written against the literals.
