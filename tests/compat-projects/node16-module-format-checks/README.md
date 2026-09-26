# node16-module-format-checks

Module-format rules under `module: node16`: a CommonJS file importing an
ECMAScript module synchronously (TS1479, TS1471 for `import = require`), a
type-only import of one without a `resolution-mode` (TS1541), top-level
`await` (TS1309) and `import.meta` (TS1470) in a CommonJS file, and the
`<T>() =>` arrow reserved in `.mts`/`.cts` files (TS7060).
