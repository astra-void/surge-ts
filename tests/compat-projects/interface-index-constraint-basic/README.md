# interface-index-constraint-basic

tsc's `checkIndexConstraintForProperty` for an interface's own members:
every property must be assignable to the string index signature's type, and a
numerically named one to the number index signature's — TS2411 on the
property. An optional member keeps its `undefined` (tsc strips it only under
`exactOptionalPropertyTypes`), and a method is compared by its function type.
