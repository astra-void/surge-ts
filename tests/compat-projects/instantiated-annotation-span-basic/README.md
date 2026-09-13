# instantiated-annotation-span-basic

A generic call re-resolves the callee's written parameter and return annotations
under the call's substitution. That is not a fresh declaration check: the
declaration was already checked where it was written, and it was checked against
the type parameter's **constraint**. Anything raised during re-resolution is
therefore raised at the declaration's span, from a call site that cannot see it
and cannot fix it.

`K[1]` is the concrete case. At the declaration it is legal — index 1 of
`[string, number?]` exists, optional — and tsc never re-checks it against the
arity of whatever `K` is instantiated with. surge re-resolved it under
`K = [""]` and reported `TS2493` on the callee's parameter list, twice over:
once for a plain declaration, once for a generic arrow whose callback parameter
names it.

Diagnostics raised while instantiating are now discarded. A written out-of-range
index on a *concrete* tuple — the diagnostic tsc does emit — is unaffected,
since that is checked where it is written; the Rust test
`a_written_out_of_range_tuple_index_still_reports` is the control for it.

`fromTheWrapper` is tanstack-query's `useWrappedQuery` reduced to its parameter
list.
