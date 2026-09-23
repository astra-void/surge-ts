# type-only-export-value-use

tsc resolves an alias through every declaration in its chain and keeps the first
type-only one it passes (`markSymbolOfAliasDeclarationIfTypeOnly`,
`resolveIndirectionAlias`, `getExportsOfModuleWorker`'s `typeOnlyExportStarMap`).
A value use of such an alias is TS1362 when that declaration is an
`export type { … }` or `export type *` (TS1361 for an `import type`), and the
value's type still resolves — `new A()` reports nothing else.

surge recorded a type-only export as a type with no value, so the same uses
were TS2693 ("only refers to a type"), and a plain re-export of a type-only
export was a false TS2305. A name the module also exports directly
(`export class C` next to `export type *`) stays a value, and the module
namespace object leaves type-only exports out (`ns.A` is TS2339).
