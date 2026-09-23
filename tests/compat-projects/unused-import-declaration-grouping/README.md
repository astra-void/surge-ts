# unused-import-declaration-grouping

With `noUnusedLocals`, an import declaration of several bindings that are all
unused reports once, TS6192 on the whole declaration (tsc
`reportUnusedImports`). A declaration with one binding read reports TS6133 per
unused binding, a single-binding declaration reports TS6133, and an imported
name starting with `_` is exempt.
