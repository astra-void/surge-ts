# function-namespace-merge-members

A function and the namespaces of the same name are one symbol (tsc's
`getTypeOfFuncClassEnumModule`): the value is callable and carries every
namespace block's exported members, including after the function declaration
itself is checked. A member no block exports is TS2339, and `new` on it resolves
the call signature (`resolveNewExpression`): TS7009 under `noImplicitAny`.
