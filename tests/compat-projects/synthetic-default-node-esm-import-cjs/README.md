# synthetic-default-node-esm-import-cjs

Under a node module kind, tsc's `canHaveSyntheticDefault` answers from the
two files' formats first: an ESM file's default import of a CommonJS-format
file always binds the module namespace (`counter` is not callable, its
`default` is a member), while between two ESM files there is no synthetic
default (`esm.mts` has no default export, TS1192). A CommonJS importer
(`cjsMain.cts`) still gets the real default of a TypeScript file.
