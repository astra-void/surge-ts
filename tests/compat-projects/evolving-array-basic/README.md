# evolving-array-basic

tsc types an un-annotated `[]`-initialized local under `noImplicitAny` by its
mutations (`autoArrayType`): `push`, `unshift` and `x[n] = v` add element
types as control flow reaches them, a loop's mutations reach every read in its
body, and a read with no element type yet is an implicit `any[]` — TS7005 on
the read, TS7034 on the declaration. A closure over a `const` cannot see the
mutations. An empty array literal anywhere else is `never[]`, which a union
with another array absorbs (`list || []`).
