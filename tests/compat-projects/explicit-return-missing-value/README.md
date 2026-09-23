# explicit-return-missing-value

tsc's `checkAllCodePathsInNonVoidFunctionReturnOrThrow`: an annotated function
whose end point is reachable reports TS2355 when it has no explicit `return`
at all, before TS2366 (under `strictNullChecks`) and TS7030 (under
`noImplicitReturns`). "Explicit return" is the binder's `hasExplicitReturn`,
which only `bindReturnStatement` sets: any `return`, with or without a value,
but not a `throw`, and not a `return` in code no flow reaches (the binder binds
an unreachable statement without it). surge counted a `throw` as a value
return, so a function that only throws on some paths reported TS7030/TS2366
instead of TS2355.
