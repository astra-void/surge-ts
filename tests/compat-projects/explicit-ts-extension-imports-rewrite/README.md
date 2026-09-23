# explicit-ts-extension-imports-rewrite

`rewriteRelativeImportExtensions` permits TypeScript-extension imports the
way `allowImportingTsExtensions` does (tsc's `GetAllowImportingTsExtensions`),
so `./value.ts` resolves without TS5097 and the binding is typed (TS2322).
