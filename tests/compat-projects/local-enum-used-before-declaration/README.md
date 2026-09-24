# local-enum-used-before-declaration

An enum declared in a function body is read before its declaration as tsc's
TS2450 (`checkResolvedBlockScopedVariable`), not as a variable's TS2448 and
TS2454; a `const enum` has no binding to be early of and is not reported, and
a read in a nested function runs later and is fine.
