# rest-tuple-literal-context-basic

A tuple with a rest slot (`[number, ...string[]]`) is tuple-like, so tsc's
`checkArrayLiteral` types an array literal checked against it as a *tuple*.
surge had contextual typing for a fixed tuple only: against a rest tuple the
literal was inferred as an array, and `(number | string)[]` rightly does not
satisfy `[number, ...string[]]` — so every valid literal was a false TS2322,
and a callback in a rest slot got no parameter types.

On failure tsc's `elaborateArrayLiteral` reads the target's element type by
index, which only a *leading* fixed slot answers: a wrong leading element is
reported on the element. A wrong rest or trailing element, a literal shorter
than the fixed slots, and an excess property nested in a rest slot are all the
whole value's failure — on the declared name, as TS2345 for an argument, or on
the excess property itself.
