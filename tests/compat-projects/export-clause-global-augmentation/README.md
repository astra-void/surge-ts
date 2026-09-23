# export-clause-global-augmentation

tsc's `checkExportSpecifier` reports TS2661 only for a name whose first
declaration's container is a global source file. A global augmentation
(`declare global { … }`, or `global { … }` inside `declare module "x"`) is
merged into the globals after every script file's own declarations, so a
name only an augmentation declares is declared in that block: exporting it is
a local export that importers read (`@types/node` re-exports its buffer aliases
from `node:buffer` this way). A name a script file also declares at its top
level (`scriptGlobal`, `ScriptAndAugmented`) keeps TS2661. surge reported
TS2661 for every global.
