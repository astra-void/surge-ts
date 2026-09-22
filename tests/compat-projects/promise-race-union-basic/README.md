# promise-race-union-basic

`Promise.race([a, b])` resolves to what any one element awaits to — the union
the lib's `Promise<Awaited<T[number]>>` names over an array-literal argument.
surge now answers that union directly instead of instantiating the lib
signature. In the default configuration the lib signature already produced it,
so this preset guards that nothing regresses there; under
`SURGE_PROMISE_NOMINAL=1` the lib path left `Awaited<T[number]>` unevaluated
and the whole race untyped. An element's fresh literal widens as an inferred
type argument does.
