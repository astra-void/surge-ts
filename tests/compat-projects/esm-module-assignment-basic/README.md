# esm-module-assignment-basic

With `module` in ES2015..ESNext, `import x = require(...)` cannot be emitted
(TS1202) and neither can `export =` in a file that is not CommonJS (TS1203),
per tsc's `checkImportEqualsDeclaration`/`checkExportAssignment`. A type-only
`import =` is fine.
