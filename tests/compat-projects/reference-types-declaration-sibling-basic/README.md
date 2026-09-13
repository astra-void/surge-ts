# reference-types-declaration-sibling-basic

`/// <reference types="pkg" />` resolved a package with no `types` field to its
`index.ts` rather than its `index.d.ts`. An extensionless entrypoint probes
`.ts` before `.d.ts`, which is right for *module* resolution — tsc prefers a
shipped source there — but a type reference directive is resolved with
`Extensions.Declaration` and never reaches a `.ts`.

For a package that ships both flavors of one surface the difference is the whole
point of the directive. `@cloudflare/workers-types` writes
`declare abstract class D1Database` in `index.d.ts`, a script, so the class is a
global; `index.ts` writes the same declaration with `export`, making it a module
where nothing is global. Taking the source resolved the directive — no `TS2688`
— and then contributed no globals at all, which is where drizzle-orm's 15
`TS2304` on `D1Database`, `SqlStorageCursor` and their siblings came from.

`describe` is the case that was broken. `theGlobalIsTheDeclarationFlavor` is the
intentional error and pins *which* file won: `retries` is on `VendorOptions`,
not on `VendorHandle`, so the diagnostic proves the declaration flavor is the
one in scope rather than some merge of the two.
