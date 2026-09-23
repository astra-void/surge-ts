# ambient-module-import-equals-namespace

`import lib = require("lib2")` where `"lib2"` is an ambient `declare module`
without `export =`: tsc's `getTargetOfImportEqualsDeclaration` resolves the
module symbol itself (`resolveExternalModuleSymbol` finds no `export =`), so
`lib` is the module's namespace. Its exported types are `lib.D` and
`lib.N.I`, its value is `typeof import("lib2")`, and `lib` written as a type
is TS2709. surge bound an unknown placeholder for any module without a file,
so `lib.D` degraded and `typeof lib` was a false TS2304.
