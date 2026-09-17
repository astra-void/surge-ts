# unannotated-method-return-type-basic

An unannotated method returns what its body returns (`getReturnTypeFromBody`),
but surge typed every such method on the instance side as returning `any`. So a
method overriding or implementing a member with an incompatible return was not
TS2416, and nothing read from a call to it was checked.

The instance side is built from written types before bodies are checked, so the
return is lowered from syntax where it is evident: literals returned by every
`return` widen to their primitives, and a method returning no value is `void`
(an `async` one is `Promise` of either). Any other returned value leaves the
method returning `any`, as before.
