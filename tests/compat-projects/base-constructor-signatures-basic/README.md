# base-constructor-signatures-basic

Two places tsc reads the base class's construct signatures.
`resolveCallExpression` resolves a `super(…)` call against them like any other
call — arity, argument types, overloads, contextually typed callbacks — and
`getDefaultConstructSignatures` gives a derived class that declares no
constructor the base's signatures, returning the derived instance. surge
walked `super(…)` arguments against nothing and let a constructor-less derived
class accept any argument list, which also left a callback argument without a
contextual type (a false TS7006).
