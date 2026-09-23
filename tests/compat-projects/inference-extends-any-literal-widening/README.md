# inference-extends-any-literal-widening

tsc's `getConstraintFromTypeParameter` reads a written `extends any` as the
constraint `unknown`, and an `unknown` constraint is no literal context: it has
no primitive kind (`hasPrimitiveConstraint`), and its apparent type `{}` gives
an object literal's members no contextual type that could keep a literal. So
`pickAny({ x: 3 }, { x: 6 }, { x: 6 })` infers `{ x: number }` exactly as an
unconstrained parameter does. surge treated every written constraint as a
literal context and inferred `{ x: 3 } | { x: 6 }`.

A constraint that does name literal members (`T extends { x: 1 | 2 }`) still
keeps them. The two intentional errors assign the widened results to
`{ x: 3 }` and are tsc errors too.
