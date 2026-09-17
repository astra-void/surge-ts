# missing-return-diagnostic-selection-basic

`checkAllCodePathsInNonVoidFunctionReturnOrThrow` (checker.go:3751) is a
four-way choice, and surge collapsed it to two. Three of the four branches were
wrong:

- The early exit is `maybeTypeOfKind(t, Void)`, so a **union** carrying `void`
  (`number | void`) is exempt just like bare `void`. surge only exempted the
  bare keyword and reported TS2366/TS2355 on the union.
- A `never` annotation with a reachable end point is **TS2534**, not TS2355.
- TS2366 is gated on `!isTypeAssignableTo(undefinedType, t)`. A return type that
  already admits `undefined` does not get it — under `noImplicitReturns` the
  case falls through to TS7030 instead, which surge never reached for an
  annotated function.

The exempt set is also not "anything unresolved": a written `unknown` is
reported, while surge's `Type::Unknown` degradation sentinel must stay silent.
`GenuineUnknown` is what separates them.

`number | null` is deliberately absent from the fixture: surge models `null` as
`Type::Undefined`, so it cannot yet tell `number | null` (TS2366) from
`number | undefined` (nothing) apart. That one waits on a real `null` type.
