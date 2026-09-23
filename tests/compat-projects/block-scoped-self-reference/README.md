# block-scoped-self-reference

tsc's `isBlockScopedNameDeclaredBeforeUse` through
`isImmediatelyUsedInInitializerOfBlockScopedVariable`: a `let`/`const` read
inside its own declaration's initializer, or a `for…in`/`for…of` head's binding
read in the expression the loop iterates, is used before its declaration
(TS2448) — at the top level, in a block, in a function body and in a
namespace alike. The head's binding is in scope there, so the name resolves
instead of reading as missing (TS2304).

A read of an unannotated binding is circular, so its type is `any` and no
TS2454 goes with it (TS7022 under `noImplicitAny`); an annotated one reads at
its declared type, which is unassigned there unless it admits `undefined`. A
read from a nested function is deferred and legal, and a `var` has no
temporal dead zone.

A write is positional too: an assignment expression whose target is in its
temporal dead zone is TS2448 as well, and tsc still checks the write itself —
`const c = (c = 1)` is TS2588 beside it.
