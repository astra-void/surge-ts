# async-implicit-return-awaited-basic

An unannotated async function returns the awaited value of what it `return`s
(tsc's `getReturnTypeFromBody`), so under `noImplicitReturns` a returned
`Thenable<void> | null` counts as void-like and the fall-through is fine
(`isUnwrappedReturnTypeUndefinedVoidOrAny`), while `return 1` still owes TS7030.
