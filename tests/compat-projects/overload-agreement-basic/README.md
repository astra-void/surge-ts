# overload-agreement-basic

tsc's `checkFunctionOrConstructorSymbol`: overloads must agree on `export`
(TS2383), ambience (TS2384), accessibility (TS2385), optionality (TS2386) and
`abstract` (TS2512), measured against the implementation. A bodyless
declaration followed by a static/instance twin is TS2387/TS2388, and one
followed by a differently named implementation is TS2389 — neither is the
TS2391 "implementation missing" surge used to report for both.
