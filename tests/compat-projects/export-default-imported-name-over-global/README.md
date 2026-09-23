# export-default-imported-name-over-global

`export default Error` exports the entity the name resolves to from the
module, which tsc's `resolveEntityName` reads from the module's own
declarations, then its imports, then the globals. `Error` is imported here,
so the default export is the imported class — next's `next/error.d.ts` has
exactly this shape. surge typed the default expression without the import
bindings and exported the global `ErrorConstructor`, reporting TS2339 for
`code` and missing the TS2322.
