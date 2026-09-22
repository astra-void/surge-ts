# dependent-object-destructuring-basic

The names of `const { kind, payload } = action`, or of a destructured
parameter, are dependent when the source is a discriminated union: testing
`kind` retypes `payload` (tsc treats the binding as the property access it
stands for). surge had this only for array patterns and truthiness, so
`payload` stayed the union of every member's and its uses were false TS2339s
and TS2322s. Holds for an `if`, a conditional expression, a `switch`, and a
source that is a call. Names bound one at a time (`const kind = action.kind`)
and a name with a default are not dependent.
