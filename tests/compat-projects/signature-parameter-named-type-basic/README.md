# signature-parameter-named-type-basic

A call signature, method signature or function type parameter written as a
bare name that is a type keyword or names a type in scope (`(string) => void`)
is TS7051 with the suggested `argN: Name` under `noImplicitAny` (tsc's
`reportImplicitAny`). Construct signatures, constructor types and function
declarations keep the plain implicit `any` (TS7006); an unresolved name is
TS7006 too.
