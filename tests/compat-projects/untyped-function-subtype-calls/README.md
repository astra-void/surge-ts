# untyped-function-subtype-calls

tsc's `isUntypedFunctionCall`: a callee with no call or construct signature
that is still assignable to the global `Function` — `Function` itself, an
interface extending it, or an instance of a class extending it — is called
untyped. The call returns `any`, and written type arguments are TS2347. An
object that is not a `Function` stays not callable (TS2349).
