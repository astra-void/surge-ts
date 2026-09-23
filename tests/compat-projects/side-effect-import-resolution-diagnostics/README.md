# side-effect-import-resolution-diagnostics

tsc checks an unresolved side-effect import (`import "./x";`) through the
same `resolveExternalModule` branches as any import, with TS2882 as the
not-found message its caller passes (checker.go `checkImportDeclaration`):
a `.json` specifier under `resolveJsonModule: false` is TS2732, and an
extensionless relative specifier resolved in node16/nodenext ESM mode is
TS2834/TS2835. surge reported TS2882 for every unresolved side-effect import.
Upstream `sideEffectImports3` hits this when its directory sits in a
`"type": "module"` package scope.

- `src/index.ts` is ESM (`"type": "module"`): `./not-a-module` is TS2835
  (`./not-a-module.js`), `./missing` is TS2834, `./data.json` is TS2732;
  `./missing.js` and the Node built-in `fs` stay TS2882 (no install-@types
  hint for side-effect imports), and `./not-a-module.js` resolves.
- `src/cjs/index.ts` is CommonJS: `../not-a-module` resolves to the script
  file with no diagnostic, and `./missing` is TS2882.
