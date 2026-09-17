# call-callee-and-arity-arguments-basic

tsc's `resolveCallExpression` reads the callee through
`checkNonNullExpression`, so calling a possibly-`undefined` function is
TS2722 on the callee and the call then proceeds on its defined part; a
member that is not callable is TS2349 at the member name. A call rejected
for its argument count still has every argument checked (an unresolved name
inside is TS2304, an excess callback's parameter TS7006). surge reported
TS2349 for the first, anchored the second on the whole call, and skipped the
arguments of the third.
