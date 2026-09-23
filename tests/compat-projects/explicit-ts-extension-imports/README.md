# explicit-ts-extension-imports

A relative specifier that ends in a TypeScript extension resolves through
tsc's `tryAddingExtensions` matrix: `.ts`/`.d.ts` probe `.ts`, `.tsx`, `.d.ts`;
`.tsx` probes `.tsx`, `.ts`, `.d.ts`; `.mts`/`.d.mts` probe `.mts`, `.d.mts`.
The imports are typed (TS2322 on `flavor`), and a missing file is still
TS2307. A `.d.ts` specifier in an emittable import is TS2846, whose suggestion
is the TypeScript source here because `allowImportingTsExtensions` is on and
the output is an ES module; `import type` is exempt.
