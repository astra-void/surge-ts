# implicit-this-any-basic

> **Withheld — not emitted.** This check is `catalog-only`: an object-literal accessor under a contextual type reaches the check through a path that does not clear the implicit-`this` flag, so zod's `const def: core.$ZodObjectDef = { get shape() { … this.shape … } }` was a false positive. Re-land it by carrying the `this` binding on the lowered arrow itself.
> The fixture and the analysis below are kept for whoever re-lands it;
> the preset is unregistered so the sweep does not compare it.

A `this` in a plain `function` with no `this` parameter has no type, and tsc
reports reading it (TS2683). Nothing reported it.

The case that decides whether a port is faithful is `throughArrow`: an arrow
does **not** bind `this`, it inherits the enclosing function's, so
`function f() { return () => this }` is reported. Assuming "inside an arrow is
safe" turns that into a miss. The exemptions are pinned for the same reason:
a `this` parameter (written or ambient), an object-literal method or getter, a
class method or field, and module top level all give `this` a binding.

Two mechanics were needed to get there. oxc keeps a `this` parameter out of the
parameter list entirely (`Function::this_param`), so its presence is carried on
the parsed declaration — without that, `annotatedThis` was a false positive.
And surge lowers an object-literal method to an arrow, which would otherwise
make it inherit an enclosing function's implicit `this`; the lowering site
clears the flag.

tsc gates this on `noImplicitThis`, which surge does not model; it rides
`noImplicitAny`, and both derive from `strict`.

**Not covered**: a `this` inside a **function expression**
(`const f = function () { return this }`) or inside a **nested function
declaration**. Neither body is checked by surge at all — `FunctionExpression`
has no arm in the expression parser, and a nested function declaration does not
reach the body checker — so *no* diagnostic fires in them, not just this one.
That gap is much larger than this rule and is deliberately left visible rather
than worked around here.
