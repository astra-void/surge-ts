# new-error-type-callee

tsc's `resolveNewExpression` resolves a target of the error type as an error
call (`resolveErrorCall`): the arguments are still checked, but nothing is
reported about the target. Without noImplicitAny an array read by a
non-numeric string key is such an unresolved element, so constructing what
it holds is fine.
