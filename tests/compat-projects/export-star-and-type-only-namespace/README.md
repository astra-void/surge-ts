# export-star-and-type-only-namespace

`export *` of a module whose exports are its `export =` is TS2498
(`checkExportDeclaration`). A namespace imported with `import type` is a module,
so a value use of it is TS1361 whether or not the module resolves, and an
unresolved one is still TS2307; type positions read it freely.
