# for-header-clauses-basic

The init expression and the update clause of a `for` header were lowered as
bare expressions, which related nothing: an assignment written there was
never checked against its target, and a compound one was not evaluated at
all. They are lowered like the statements they would be on their own, one per
comma operand. The update clause runs in the header's scope, so a `const` the
body declares does not shadow the header's there, and an assignment in the
init counts as one (`for (k = 0; …)` no longer reads `k` as unassigned).

What a bare block assigns to an outer binding holds after it: the narrowing
used to die with the block's scope, which the loop join after a `for` with an
update clause depends on. A binding the block declares itself stays its own.
