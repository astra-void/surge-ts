# undecidable-conditional-never-branch-basic

Passes on the default path and **fails under `SURGE_GENERIC_RECURSIVE_ALIAS=1`**,
which is what it is here to pin: the gate resolves the recursive alias, and the
answer it then computes is wrong.

`ParsedType` has no rest element, so the parser lowers a variadic tuple pattern
(`[...infer R, unknown]`, which is all `DropLast` is) to `ParsedType::Unknown`.
That is a *clean* sentinel, so the conditional's taint guard does not see it and
the `extends` test answers `false`, taking the `: never` branch. `never` is
absorbed by any union around it, so

    type TuplePrefixes<T> = T extends readonly [] ? readonly []
                                                  : TuplePrefixes<DropLast<T>> | T

collapses from the prefix union to its last member, and assigning a genuine
prefix is rejected. tsc accepts all three assignments.

## What was tried

Making an undecidable conditional degrade instead of answering `never` closes
this and the one tanstack-query false positive it causes. It is **not** viable
as written: a degraded result is uncacheable by design, so every such
conditional is re-resolved at each use and tRPC stops terminating (>560s,
against 9s). Widening the rule to every structureless `extends` also cost zod
two and ofetch one false positive. Both measured, both reverted.

The repair is to represent rest elements rather than to degrade around them.
That would also give `Head`/`Tail`/`Last`/`Init`, which are all silently
unmodelled today, and it pairs with the missing `ParsedType::Tuple` arm in
`bind_infer_captures` — `[infer A, infer B]` binds nothing either, so every
fixed-position tuple capture is unmodelled as well.
