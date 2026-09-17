# string-indexed-access-type-basic

An indexed access type on a string reads its apparent type: `K[number]` for
a string or string literal (or a union of them) is `string`, and
`"abc"["length"]` is the `String` member. surge reported a false TS2538 or
TS2339 for these.
