# alias-double-negation-and-basic

`!!x` is `x`'s truthiness spelled out. As the whole condition it already
narrowed, and so did `x !== undefined && …`; but as an `&&` operand — written
inline or reached through a boolean `const` alias — the reference-guard walk
had no case for the double negation, so `!!query && query.isFetched()` proved
nothing about `query` in the branch and `query.setState(...)` reported
`TS18048`. tanstack-query's `streamedQuery` writes exactly the aliased form.

The walk now unwraps `!!` and hands the inner reference to the same guards the
bare reference gets. `negated` is the control: a single `!` proves the opposite,
and reading through `query` there is the real error, expected with
`@ts-expect-error`.
