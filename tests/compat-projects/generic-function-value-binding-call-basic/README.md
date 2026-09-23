# generic-function-value-binding-call-basic

A binding initialized with a generic function (`const alias = identity`) is
that function's type, type parameters and all, so a call through it infers
them from its arguments like a call of the declaration itself. surge gave the
new binding no signature, and every such call returned the signature with its
type parameters still open (`[T]`).
