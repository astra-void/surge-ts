# namespace-require-alias

An `import x = require("m")` inside a namespace is TS1147, and the program
never collects it: resolving the alias (tsc's `resolveExternalModule`) finds
only an ambient `declare module "m"` (`tryFindAmbientModule`), so the ambient
module types `lib` (TS2322), while `./not-there` and even the existing
`./real` are TS2307 at their specifiers — reported when the alias is
resolved, i.e. once something names it, so the unused alias reports nothing.
The aliases are declared either way, so their uses are not TS2304.
