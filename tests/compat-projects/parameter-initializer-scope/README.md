# parameter-initializer-scope

tsc's `resolveName` for a name written in a parameter initializer: the lookup
reaches the function's locals with `lastLocation` a parameter, so every
parameter of the list is visible — itself and the ones declared after it
included — while a `var`/`let` the body declares is skipped
(`useOuterVariableScopeInParameter`) and the enclosing scope answers. A
parameter's type is resolved on demand when another reads it, so a later
parameter reads at its own type; an eager read that closes a cycle is a
circularity, `any` and TS7022 under `noImplicitAny`, and a read from a nested
function is deferred and never one. The script half checks the same rule
before the script's globals and the lib's are declared.

The TS2372/TS2373 reads, the TS2339 on a later parameter's member, the TS2322
through a deferred read, and the TS7022 cycles are the intentional errors, and
all are `tsc` errors too.
