# reexported-namespace-object-members-basic

A module namespace that one module re-exports (`import * as inner from
"./inner"; export { inner }`) keeps its members' real types for a consumer two
levels down.

The object an importer binds is materialized at import-binding time, and the
bindings the final analysis round runs under come from the preliminary
analyses, where the thin value pass degrades every variable to `unknown`. A
module that only *uses* the import re-resolves later; one that re-exports it
baked that thin object into its own export table, and nothing refreshed it — so
`middle.inner.thing` was open, and every read off it was silent. The namespace
objects are now rebuilt against the final export tables before the check
phase's import bindings are collected.

`@types/jscodeshift` reaches its whole builder surface this way, through
recast's re-export of the `ast-types` namespace.
