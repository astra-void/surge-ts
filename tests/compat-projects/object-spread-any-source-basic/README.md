# object-spread-any-source-basic

`{ ...anyValue, k: v }` is `any` in tsc — the spread can carry anything, so the
literal has no knowable shape. Surge skipped a non-object spread source and
closed the literal over the remaining properties, which reported every member
the callee requires as missing (`persistQueryClientRestore({ ...persistOptions,
queryClient })` in tanstack's persist client, once the parameter type resolved
cleanly instead of degrading open).

A spread whose source is surge's own degradation sentinel keeps the literal
open instead, since the members it stands for are real but unenumerable.
