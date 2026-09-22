# falsy-branch-narrowing-basic

Where a reference tested falsy (`if (!x)`, the `else` of `if (x)`, after an
early exit, the false arm of `x ? :`), tsc leaves only what can be falsy
(`TypeFacts.Falsy`): a primitive keeps its type, `boolean` is `false`, the
nullish members and the falsy literals stay, and objects, functions, arrays
and truthy literals go — `never` when nothing is left. surge narrowed only the
truthy side. Calling a `never`-typed callee is not an error.
