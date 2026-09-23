# reference-types-resolution-mode

A `/// <reference types>` directive resolves in the mode its
`resolution-mode` attribute names, else in its file's default mode (tsgo's
`getModeForTypeReferenceDirectiveInFile`), and that mode picks the package's
`import` or `require` condition. surge always resolved with `import`.

- `src/esm/explicit.ts` asks for the `require` face in an ESM file.
- `src/cjs/implied.ts` is CommonJS (`src/cjs/package.json`), so its bare
  directive takes the `require` face; `src/esm/implied.ts` takes `import`.
- `esm-pkg` exports only an `import` condition: the CommonJS directive in
  `src/cjs/rejected.ts` is TS2688, while `src/esm/accepted.ts` loads it.
