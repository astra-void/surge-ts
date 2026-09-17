# var-redeclaration-type-basic

A `var` may be redeclared, but every declaration has to give it the same type
(TS2403). tsc compares them for *identity*, not assignability, and anchors on
the second declaration's name. An identical redeclaration is legal and is
pinned as such.

Both the module scope and a function body are pinned: the two go through
different entry points in the variable checker, and the first version of this
rule sat in a wrapper only the module path uses, so a function-local `var`
silently escaped it.

Two restrictions keep this free of false positives:

- The lookup is own-scope only, so a module `var` that shadows a same-named
  ambient global is not a redeclaration.
- Both declarations must be annotated. tsc compares widened *declaration*
  types, which for an unannotated `var` come from its initializer; surge's
  inference there is not faithful enough to report on.

An ambient `declare var` pair is **not** pinned: ambient declarations are
pre-registered before this check runs, so a declaration would find its own
registration and read as a redeclaration of itself. The duplicate `let`/`const`
check skips ambients for the same reason.

The declarations are deliberately left unread. Reading an unassigned `var`
brings in TS2454, which surge reports for a block-scoped binding but not for a
`var` — an unrelated gap that would otherwise sit in this fixture.

