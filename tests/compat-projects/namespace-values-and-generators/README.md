# namespace-values-and-generators

A namespace's value has its exported members only, a function as its written
signature, a class as its constructor and a literal-initialized variable as
the literal's declared type. A module-level `let` with no initializer is
typed by flow, starting from `undefined`. `delete` sees through an optional
chain to the member it removes. An ambient class's non-private member
parameters are implicitly `any` (TS7006). A member one union constituent
declares protected and another declares elsewhere is no member of the union.
A generator annotated with a return type checks what each `yield` (and
`yield*`) produces against its yield type, awaited in an async generator, and
a constructor called without `new` is TS2348.
