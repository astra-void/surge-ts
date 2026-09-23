# union-tuple-rest-contextual-parameters

A callback contextually typed by `(...args: A | B)` with tuple members takes
each parameter from that position of every tuple (tsc's `getTypeAtPosition`,
`undefined` past a short tuple), and its parameters narrow one another as a
destructuring does (`getNarrowedTypeOfSymbol`): `kind === "text"` makes `value`
a `string`. Without a test the parameter is the union (TS2339), and once any
parameter is assigned in the body none of them narrows the others (TS2322).
