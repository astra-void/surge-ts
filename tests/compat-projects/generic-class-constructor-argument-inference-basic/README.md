# generic-class-constructor-argument-inference-basic

`new C(arg)` took a class type parameter's declared *default* instead of
inferring it from the constructor's arguments, so TanStack Query's
`MutationObserver<…, TVariables = void>` stayed at `void` and every
`mutation.mutate('input')` after it was a false TS2345.

The options interface pins the two things the inference has to cross: the member
that carries `TVariables` is declared on the *base* interface, and its type is an
alias (`MutationFunction<TData, TVariables>`) that only the declaring file has in
scope. `empty.mutate()` pins the other direction: with nothing to infer from, the
declared default still applies.
