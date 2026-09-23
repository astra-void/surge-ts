# import-declaration-modifiers-basic

`export import { … } from "…"` is an import declaration carrying a modifier,
which tsc's `checkImportDeclaration` rejects as TS1191 at the `export`. The
import still binds, so the uses after it are checked against the imported
types.
