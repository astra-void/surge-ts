# function-type-parameter-shadows-file-type

tsc's `resolveName` walks a function body's own scope and the function's type
parameters before the file's declarations, so inside `clash<X>` a body-local
alias reads the type parameter `X`, not the file's `interface X`, and inside
`localShadows` the body's `X`, `Y` and `Z` shadow the file's. surge consulted
the file's declaration table before the body's layers, so each of these read
the file's type: `let c: C = arg` was "Type 'X' is not assignable to type
'X'", and `X extends [infer A] ? A : never` evaluated against the interface to
`never`. Without a clash (`noClash`, `readsFileType`) nothing changes; the two
intentional errors read the file's `X` and the body's `Z`.
