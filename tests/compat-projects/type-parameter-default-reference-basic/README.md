# type-parameter-default-reference-basic

A type parameter's default may only reference the type parameters declared
before it (TS2744, tsc's `checkTypeParametersNotReferenced`). A name a nested
signature, `infer` capture or mapped-type key redeclares is not the outer
parameter, and constraints are not checked.
