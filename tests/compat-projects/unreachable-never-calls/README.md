# unreachable-never-calls

A call ends the flow when the binder made it a `FlowCall` (a whole
expression statement, or a comma operand, with a dotted-name callee) and
`getEffectsSignature` — which only takes declared types, through
`getTypeOfDottedName` — finds a signature returning `never`, or asserting a
parameter the call passes a false argument for. Under
`allowUnreachableCode: false` what follows is TS7027. A function, an
overload set, an annotated variable or parameter, a namespace export and a
method reached through `this` or `super` all count; a variable with no
annotation, a local that shadows the declaration, an `asserts` call with a
true argument and a call in an initializer do not.
