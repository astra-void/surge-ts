# arbitrary-extension-imports-allowed

With `allowArbitraryExtensions`, `./component.html` resolves to the
declaration file `./component.d.html.ts` from any importer, so both the
direct import and the declaration file's re-export are `string` (TS2322
twice).
