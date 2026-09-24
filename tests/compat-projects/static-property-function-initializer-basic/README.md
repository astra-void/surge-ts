# static-property-function-initializer-basic

An unannotated static property initialized with an arrow or function expression
has that function's own signature as its type (tsc checks the initializer,
`getTypeForVariableLikeDeclaration`, and a function expression checks to
`getSignatureFromDeclaration`), exactly like a static method with the same
parameters and return type — so calls through it check their arguments and
reads of it are not `any`.
