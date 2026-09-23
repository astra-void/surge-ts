# resolution-mode-from-emit-format

Under `moduleResolution: bundler` the resolution mode of an import still
follows the importing file's emit format (tsgo's `getModeForUsageLocation`):
with `module: commonjs` a `.ts` file emits `require` calls, so its imports —
and every `import x = require()` — resolve with the `require` condition, while
an `.mts` file resolves with `import`. The project's `"type": "module"` does
not count for a file outside `node_modules` under bundler.

- `src/imports.ts` gets `dual`'s `require` face (TS2305 for `fromEsm`) and
  cannot reach the import-only `esm-only` (TS2307).
- `src/requires.ts` reads `fromCjs` through `import dual = require("dual")`.
- `src/real-imports.mts` gets the `import` face (TS2305 for `fromCjs`). Its
  successful `esm-only` resolution must not stand in for `imports.ts`'s
  failed one.
