# missing-return-loose-null-checks

A function whose end is reachable but that returns a value on some path is
TS2366 only under `strictNullChecks` (tsc's
`checkAllCodePathsInNonVoidFunctionReturnOrThrow`); without it `undefined`
fits every return type and `noImplicitReturns` reports TS7030 instead. With no
value return at all it is TS2355 either way.
