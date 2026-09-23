# arbitrary-extension-imports

`./component.html` resolves to `./component.d.html.ts` (tsc's
`tryAddingExtensions` for an extension it does not know), but outside a
declaration file that resolution needs `allowArbitraryExtensions`: without it
the import is TS6263 and binds nothing typed. The declaration file
re-exporting from the same path is exempt, so `again` is `string` (TS2322).
