# merged-interface-type-parameter-defaults

An interface's type parameter is one symbol across all of the interface's
declarations, so its default and its constraint come from whichever
declaration writes them (`getResolvedTypeParameterDefault`,
`getConstraintDeclaration`), and `getMinTypeArgumentCount` counts a parameter
as optional when any declaration gives it a default. `@types/node` declares
`NodeJS.AsyncIterator<T, TReturn, TNext>` in `compatibility/iterators.d.ts`
and again with defaults in `globals.d.ts`, and writes
`NodeJS.AsyncIterator<string>`. surge kept the first declaration's parameters,
so each such reference was a false TS2314 and resolved to nothing.
