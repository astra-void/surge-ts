# array-filter-type-predicate-basic

`Array.prototype.filter` narrows its element type when the callback is a type
predicate: `rows.filter(isCode)` is `(Row & { identity: Code })[]`, not `Row[]`.
The predicate lives on the argument's *collected signature* rather than on its
resolved callable type — surge models `filter`'s callback as returning `any`, so
the narrowing has to be read at the call site.

Two things had to be true for the first line to check. The second is the reason
it took a fix of its own: an intersection merges same-named properties by
*intersecting* them, so `Row & { identity: Code }` narrows `identity` from
`Code | NameOnly` down to `Code`. First-operand-wins kept the union and made
every predicate narrowing through an intersection read the unnarrowed type.

The last two lines pin the edges: a plain boolean callback still yields the
unnarrowed element type, and the narrowed result is a real type — assigning
`string[]` to `number[]` still reports.
