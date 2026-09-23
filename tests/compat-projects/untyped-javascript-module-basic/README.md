# untyped-javascript-module-basic

A module that resolves only to a JavaScript file exists; it is untyped, not
missing. tsc's resolver falls back to JavaScript when no TypeScript or
declaration file answers a specifier — a relative file (extension replaced or
appended), a directory index, a package's `main` or subpath — and
`resolveExternalModule` reports TS7016 under `noImplicitAny` instead of TS2307.
A `.jsx` target while `jsx` is unset is TS6142 (`GetResolutionDiagnostic`), a
side-effect import of a JavaScript file reports nothing, a `.js` with a sibling
`.d.ts` is typed, and a specifier nothing answers is still TS2307.

Paired with `untyped-javascript-module-no-implicit-any`.
