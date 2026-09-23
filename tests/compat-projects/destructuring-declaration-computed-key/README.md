# destructuring-declaration-computed-key

A binding element under a computed key reads the source indexed by the key,
and one under a string or numeric literal key reads that property — through
the index signature for a numeric name (`isNumericLiteralName`), as `"0"` of
a string — so every name here is declared and typed: `fromUnion` is
`source["a" | "b"]`, which does not fit `string` (TS2322).

An array literal under a pattern with numeric names is contextually a tuple
(`isTupleLikeType`), so `first` and `second` take their own elements and
agree with the later `var` redeclarations (no TS2403).
