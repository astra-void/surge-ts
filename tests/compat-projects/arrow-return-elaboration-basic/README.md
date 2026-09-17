# arrow-return-elaboration-basic

tsc's `elaborateArrowFunction`: when an expression-bodied arrow with no
annotated parameters does not fit a function-typed target, the mismatch is
between the *return types* and is reported at the body expression (at the `(`
of a parenthesized body). surge reported the whole signature at the target.

Pinned as the cases the rule does not cover, which still report the whole
signature at the target: an arrow with an annotated parameter, and a block
body. An object-literal body elaborates one step further, into its member.

Reporting at the body surfaced a latent gap: surge's built-in `find` /
`findIndex` / `findLast` / `findLastIndex` typed the predicate as returning
`boolean`, while the lib's non-narrowing overload returns `unknown`, so
`items.find((item) => item.name)` became a false TS2322. Both calls are pinned
as clean.
