# module-evolving-array-basic

A `[]`-initialized binding at the top level of a module or script is an
evolving array under `noImplicitAny` just as a local is (`autoArrayType`). A
top-level read no `push`/`unshift`/element write has reached yet — counting a
mutation later in the same loop, through the back edge — is an implicit
`any[]`: TS7005 on the read, TS7034 on the declaration. A closure over a
`const`, and any top-level `function` or class member, cannot see the module's
flow; mutations inside a function are not part of it. An exported binding is
not flow-typed.
