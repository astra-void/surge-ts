# synthetic-default-import-value-basic

Under `esModuleInterop`, a default import of a module without a `default` export
names what tsc's `getTargetOfModuleDefault` resolves through
`resolveExternalModuleSymbol`: the `export =` target when there is one, and the
module object otherwise — never `any`. An `export =` of a named import
(`import { strict } from "…"; export = strict;`, the shape of `@types/node`'s
`node:assert/strict`) aliases that import's target.
