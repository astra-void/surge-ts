# ambient-module-block-import-type-query

An import inside a `declare module "…"` block is one of the block's locals (the
binder declares it in the module's symbol table), so a type query in the
block's variable annotations reads its value: `typeof lib4` for an
`import lib4 = require("lib4")` over `export =`, `typeof lib5` for an
`import * as lib5`, and `typeof lib4.count` through it. surge typed those
annotations before the block's import bindings were in reach and reported
TS2304 for each name, degrading the exported variables.
