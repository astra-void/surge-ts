# jsdoc-nullability-in-typescript

A JSDoc `T!`, `!T`, `T?` or `?T` in a TypeScript annotation is TS17019
(postfix) or TS17020 (prefix) from tsc's `checkJSDocTypeIsInJsFile`, but tsc
still reads the type: `!` means `T` and `?` means `T | null` under
strictNullChecks (`getTypeFromTypeNode`). The annotated parameters are
therefore not implicitly `any`, a function annotated `string!` returns a
`string`, and `?number` flows `null` into the TS2322 at the end.
