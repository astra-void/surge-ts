# labeled-const-declaration-context

A `const` declaration may be the labeled statement of a label written in a
block or at the top level: tsc's `allowLetAndConstDeclarations` looks past a
label (or a chain of them) to its parent. Only when that parent is an `if`, a
loop or a `with` whose whole body is the labeled declaration is it TS1156.
Each label on a declaration is TS1344 either way.
