# exports-dotted-runtime-target-basic

A package `exports` entry may resolve, under the `types` condition, to a
*runtime* JavaScript file rather than a declaration. TypeScript then strips the
runtime extension and probes the declaration that sits beside it:
`./dist/boundarykit.cjs.mjs` becomes `boundarykit.cjs.mts` and then
`boundarykit.cjs.d.mts`; a `.js` target becomes `.ts`, `.tsx`, `.d.ts`.

Published bundles almost always carry an inner dot in that name — the
`.cjs.mjs` / `.esm.js` / `.cjs.js` naming preconstruct and friends emit.
surge substituted the extension with `Path::with_extension`, which rewrites the
segment *before* the runtime one, so it probed `boundarykit.d.mts` and reported
TS2307 for a package that ships its types. `react-error-boundary` is the
real-world case: it cost seven false TS2307 in the tanstack-query corpus.

The `"."` entry also pins the nested condition object (`types` → `import`),
which is how that package spells it.
