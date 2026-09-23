# jsx-implicit-runtime-import-basic

Under the automatic runtime tsc's file loader adds an import of
`<jsxImportSource or "react">/jsx-runtime` to every `.tsx` and JavaScript file,
and `getJsxNamespaceAt` reads the JSX namespace from that module's `JSX`
export (`getJsxNamespaceContainerForImplicitImport`). The export may be an
`export import JSX = …` alias of an imported namespace (`rt`) or a renamed
re-export (`emo`). An `@jsxImportSource` pragma picks the file's own runtime.

A runtime nothing resolves is TS2875, once per file, at the first tag tsc
checks — function expression bodies are checked after the code around them,
so `first` is reported rather than the earlier tag inside `later`; the
namespace then falls back to the factory's and the global one, and with
neither every tag is TS7026.
