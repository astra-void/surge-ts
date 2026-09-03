# export-equals-default-import-type-basic

`export = Emitter` has no `default` *value* export, so a default import fell
through to the synthetic-`any` binding and never bound the type. `import Emitter
from "events"` followed by `class X extends Emitter<T>` then reported a
surge-only `TS2304 Cannot find name 'Emitter'` — three of them on trpc.

Only the **type** side is bound from the export assignment. Binding the value too
was measured and reverted: it resolves express's `export = e` to the real
namespace, whose handler overloads surge does not select, which cost ten
implicit-any false positives for the three it saved.

The last line keeps the bound type honest — it is the class's real shape, not
`any`, so a mismatch against it still reports.
