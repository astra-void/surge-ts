# optional-chain-discriminant-narrowing-basic

`x?.kind === "circle"` narrows twice: the union by its discriminant, and `x`
itself to non-nullish, since a nullish `x` reads `undefined`, which equals no
literal. surge applied the optional-chain narrowing only when the discriminant
narrowing did nothing, so on `Circle | Square | null` the member was picked but
`null` stayed, and every read inside the branch was a false TS18047/TS18048.
The negated test is pinned too: its true branch keeps `undefined`.
