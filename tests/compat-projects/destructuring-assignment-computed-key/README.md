# destructuring-assignment-computed-key

A destructuring assignment target under a computed key reads the source
indexed by the key (`source[key]`), so it is assigned like any other target:
`n` is definitely assigned (no TS2454), and `text` takes
`source["a" | "b"]`, which does not fit `string` (TS2322).
