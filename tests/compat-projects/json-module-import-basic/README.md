# json-module-import-basic

`resolveJsonModule` was parsed and ignored, and `.json` was pinned unsupported in
the relative-specifier classifier, so every JSON import reported `TS2307`.
tRPC's upgrade CLI reads its own version out of `package.json` that way.

A `.json` file holds a value, not a program: it never reaches the TypeScript
parser, and its module surface is built straight from the value's type — the
value as the default export and as the namespace object, one named export per
top-level property. Scalars widen (`"1.2.3"` is `string`, not `"1.2.3"`), an
array becomes an array of its element union, and `[]` is `never[]`, all matching
tsc.

The two intentional errors pin that the resulting type is real: a `string` is
still not a `number`, and a key the file does not have is still missing.

The missing-key error is taken on a nested object on purpose. Naming the whole
JSON object would print two surge-wide rendering differences that are not about
JSON at all: `null` prints as `undefined` (surge has no separate `null` type)
and a non-identifier key prints unquoted. The *types* are right either way —
`missing` and `dash-key` are read above without a diagnostic.
