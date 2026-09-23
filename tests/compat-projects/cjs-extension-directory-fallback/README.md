# cjs-extension-directory-fallback

A relative specifier with an extension that misses as a file keeps looking in
CommonJS mode (tsgo's `loadModuleFromFile` and
`nodeLoadModuleByRelativeName`): the whole path takes the implicit extensions,
then loads as a directory. ESM mode does neither. surge stopped at the file
lookup in every mode.

- `src/cjs.ts` is CommonJS (`"type": "commonjs"`): `./lib.json` is the
  directory `lib.json/index.ts`, `./util.js` is `util.js.ts`, and `./dir.ts`
  is `dir.ts/index.ts` — not resolved by its `.ts` extension, so no TS5097.
- `src/esm.mts` resolves the same imports in ESM mode: TS2732 for
  `./lib.json` (no `resolveJsonModule`) and TS2307 for `./util.js` and
  `./dir.ts`. Its `import required = require("./lib.json")` resolves in
  CommonJS mode and finds the directory.
