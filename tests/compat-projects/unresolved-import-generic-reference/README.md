# unresolved-import-generic-reference

An import from a module that does not resolve binds tsc's `unknownSymbol`, and
`getTypeReferenceType` turns a reference to it into the error type before any
arity check: `MyPromise<void>` reports nothing beyond the TS2307 on the import.
surge's error-typed placeholder had no type parameters, so every generic use was
a false TS2315 ("Type 'MyPromise' is not generic"). The type arguments are
still checked (`NotDeclared` is TS2304), and a real non-generic type given type
arguments is still TS2315. `import x = require("missing")` binds the same
error type in type position instead of leaving the name a value (TS2749).
