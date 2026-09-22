# uninferred-type-parameter-fallback-basic

A type parameter with no inference candidate — `create()` called without its
optional argument — is not a failed inference. TypeScript Go's
`getInferredType` (`internal/checker/inference.go`) takes the default, then the
instantiated constraint, and finally `unknown`. surge dropped the whole binding
when the parameter had neither a default nor a constraint, or when the
constraint merely *contained* a written `unknown` (`{ e?: unknown }`). The call
then returned the uninstantiated signature and every read of the result went
silent.

The negatives pin that an argument that does supply the parameter still infers
it, and that an `object` constraint accepts an object.
