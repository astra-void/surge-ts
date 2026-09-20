# string-numeric-index-basic

`String` declares `readonly [index: number]: string`, so a numeric index reads a
`string` — and under `noUncheckedIndexedAccess` that read is `string | undefined`
like any other index-signature read. surge answered the degradation sentinel for
a string receiver instead, so nothing downstream of `text[0]` was checked: the
`.toUpperCase()` in tRPC's todomvc filter list is a `TS2532` in tsc and was
silent here.

A union of string literals answers from every member, which is what the `.map`
callback above actually has in hand (`'all' | 'active' | 'completed'`). The
array case is included as the negative that already worked.
