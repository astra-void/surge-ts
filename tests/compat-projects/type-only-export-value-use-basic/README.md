# type-only-export-value-use-basic

A value import of a name its target publishes only with `export type` resolves to a type-only declaration, so a value use of it is TS1362; a name imported with `import type` is TS1361 instead (tsc's `checkTypeOnlyAliasUse`).
