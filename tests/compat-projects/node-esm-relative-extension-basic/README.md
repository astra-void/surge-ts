# node-esm-relative-extension-basic

Under `moduleResolution: nodenext`, an ESM-mode import of a relative path
whose last segment has no extension does not resolve (tsc's
`resolveExternalModule`): TS2835 names the output extension when the path
names a file, TS2834 covers a directory or a missing one. A `.cts` file
imports in CommonJS mode and is unaffected, and `./a.js` still resolves to
`a.ts`.
