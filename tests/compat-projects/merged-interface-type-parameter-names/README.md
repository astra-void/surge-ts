# merged-interface-type-parameter-names

tsc's binder declares an interface's type parameters in the merged symbol's
members, so each declaration's members resolve under its own parameter names,
a name another declaration used is the same parameter, and the merged type's
parameters are every name in order of first appearance
(`appendLocalTypeParametersOfClassOrInterfaceOrTypeAlias`). `A` therefore has
the parameters `T, U`, and `A<string, number>` reads `y` as `number`; both
declarations are also TS2428. surge kept only the first declaration's
parameters, so `U` in the second one was a false TS2304.
