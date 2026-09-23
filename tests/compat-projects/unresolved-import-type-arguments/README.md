# unresolved-import-type-arguments

A name imported from a module that does not resolve is tsc's `unknownSymbol`:
its type is the error type, so type arguments written on it are never checked
against type parameters (no TS2315) and nothing else is reported past the
TS2307 on the import.
