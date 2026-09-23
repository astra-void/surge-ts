# construct-overload-inferred-constraint

tsc checks a type argument against its constraint where the type arguments
are written: a type reference (`checkTypeReferenceNode`) or a call's explicit
type arguments (`checkTypeArguments`). Inferred type arguments are never
reported as TS2344 (`getInferredType` falls back to the constraint instead).
`new Pair(true)` resolves to the `flag` overload and reports nothing. surge
instantiated the class with the argument inferred against the first overload
and reported the constraint violation with no location at all.
