# readonly-index-signature-writes

A `readonly` index signature refuses every write that lands on it — an
assignment, `delete` or `++` through a key no member answers — with TS2542
(tsc `errorIfWritingToReadonlyIndex`; a property access resolved through the
index reports it too). The modifier comes from the declaration (a class's
static side, interfaces, inherited through `extends`, the lib's `ArrayLike`),
the enum object's reverse mapping is read-only, and a union's index signature
is read-only when any constituent's is (`string`'s
`readonly [index: number]: string` makes `string | number[]` read-only); a
union narrowed to its mutable constituent writes freely.
