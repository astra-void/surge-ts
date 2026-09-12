# alias-condition-conditional-expression-basic

`const eager = !json; const data = eager ? [] : Array.isArray(json) ? json :
[json];` types `data` without `null`: tsc narrows a conditional expression
tested on a boolean `const` by the condition the alias was written as. surge
only expanded the alias for an `if` statement, so the `?:` operands saw the
declared type and `[json]` carried `| null` into a false TS2322. tRPC's
`resolveResponse` builds its response array this way.

The function-body declaration path now records the alias condition alongside
the binding, and expression-level narrowing expands an identifier through it —
including as one operand of `&&`/`||`/`!`. Reassigning the name in a nested
block restores the outer alias afterwards, which `shadowRestores` pins.
