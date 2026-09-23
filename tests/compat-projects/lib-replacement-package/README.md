# lib-replacement-package

Under `libReplacement`, tsc reads each default lib from an installed
`@typescript/lib-*` package when one resolves (`pathForLibFile`,
`getLibraryNameFromLibFileName`): `lib.dom.d.ts` from `@typescript/lib-dom`
and `lib.dom.iterable.d.ts` from `@typescript/lib-dom/iterable`, resolved from
the config directory in CommonJS mode. surge always read the bundled libs.

- `ReplacedDom` and `ReplacedIterable` come from the replacement package.
- The bundled `lib.dom.d.ts` is replaced wholesale, so `window` is TS2304.
