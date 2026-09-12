# module-scope-if-divergence-narrowing-basic

`if (isCancel(transforms)) process.exit(0);` at module scope narrows every
statement after it: the branch cannot fall through, so `transforms` is the
predicate's complement from there on. surge's parser dropped a module-scope
`if` entirely, so the guard narrowed nothing and `transforms.sort` was a false
TS2339 on the un-narrowed union — tRPC's `upgrade` CLI is this shape.

The parser now keeps a module-scope `if` with the same branch lowering a
function body uses. The checker evaluates its condition and, when exactly one
branch diverges (a `return`/`throw`, a `never`-returning call), applies the
other branch's narrowing to the module's symbols. The branch bodies themselves
are still not checked at module scope, which is what happened to them before;
`stillWide` pins that a branch that falls through narrows nothing.
