# overload-return-selection-basic

An overload group is checked against the permissive **fold** of its members —
a position declared differently across overloads becomes the union of what they
accept — and a call that matched the fold used to take the fold's return: `any`
when the members disagreed, the first overload's for a generic group. Every
consumer of the result then saw the wrong shape, and tanstack-query's
`useQuery<string, Error>({ queryKey, queryFn })` narrowed `state.error` on the
`initialData` overload's result.

The return type is now the first overload's that the evaluated arguments
satisfy, in declaration order, which is tsc's resolution order. Nothing else
changed: the arguments are checked exactly once, against the fold, and
selection only reads the types that check produced. An earlier design tried
each overload as a full call and rolled its diagnostics back, which re-evaluated
every argument per candidate and cost +79% CPU on tRPC; this one adds no
argument evaluation and measures flat there.

Selection is stricter than the fold's assignability where tsc is, because a
wrong pick is worse than no pick. `read('a', 'utf8')` pins the weak-type rule:
`'utf8'` must not be captured by `{ encoding?: null; flag?: string } | null`,
whose properties are all optional. `maybe` pins the written-keys rule: an object
literal that never writes `initialData` cannot satisfy the overload requiring
it, whatever its members resolved to. `second` pins the callback wildcard: a
callback is typed by whichever overload is picked, so it cannot pick, and the
other arguments decide.

When no overload accepts the arguments the call keeps the fold's return, so a
no-match call reports exactly what it did before — `TS2769` is still not
emitted, which is why this project has no failing call in it.
