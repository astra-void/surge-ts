# umd-global-namespace-type-reference-basic

`export as namespace Lib` in a module makes `Lib` a global naming that module
(Go merges each file's global exports into the globals), so a module that
neither declares nor imports `Lib` still reads its types as `Lib.Ctx<number>` —
the way `@types/react` code writes `React.Context<any>` with no import. Only a
value use from a module is an error (TS2686). surge knew the name only for that
error, so every such type reference was unresolved.
