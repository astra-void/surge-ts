# export-global-name-basic

`export { x }` of a name that resolves only in the global scope (a lib or script global, `undefined`, `globalThis`) or names a primitive type is TS2661; a name that resolves nowhere is TS2304 (tsc's `checkExportSpecifier`).
