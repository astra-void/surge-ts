# block-arrow-return-basic

An unannotated arrow or function expression with a block body takes its return
type from its `return` statements (tsc's `getReturnTypeFromBody`), widened when
nothing contextual asks for the literal. surge left every such function at its
degradation sentinel, so no use of `count`, `describe` or `makeContext` was
checked.

The last case needs two rules that inference exposed: an array element read
from a declared type keeps its literal members (`envelope.result` stays a
discriminated union through `map`), and a filter callback written as an `&&`
of discriminant tests infers its type predicate when no variant survives the
false branch.
