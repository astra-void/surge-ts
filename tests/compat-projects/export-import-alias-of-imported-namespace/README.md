# export-import-alias-of-imported-namespace

`export import Alias = Internal` exports every meaning of the entity it names,
and tsc's `resolveEntityName` reads that entity from the module the way any
reference would: its own declarations, then its imports, then the globals.
`Internal` here is imported, so `Alias.Box` is the imported namespace's
member. surge looked the entity up among the module's own declarations only,
exported nothing, and reported TS2305 at the importer's `{ Alias }`.
