# explicit-ts-extension-imports-disallowed

Without `allowImportingTsExtensions` every emittable import that resolves by
its written `.ts`/`.tsx`/`.mts` extension is TS5097 (tsc's
`resolveExternalModule`), while a `.d.ts` specifier is TS2846 regardless of
the option, suggesting the emitted `.js` path for an ES module output.
