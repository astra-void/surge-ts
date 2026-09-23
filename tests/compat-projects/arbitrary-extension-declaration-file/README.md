# arbitrary-extension-declaration-file

tsc's `IsDeclarationFileName` also takes a `.ts` file whose base name carries
`.d.` — the `{name}.d.{extension}.ts` form `allowArbitraryExtensions`
resolves `{name}.{extension}` imports to — so `component.d.html.ts` is an
ambient file where an uninitialized `const` and a bodiless function are fine.
`helper.data.ts` has no `.d.` segment and reports TS1155 and TS2391.
