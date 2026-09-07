# literal-equality-narrowing-basic

`x === "lit"` on a whole binding narrowed nothing in surge: only the
discriminant form (`x.kind === "lit"`) was recognized, so a union subject kept
every member and a `string` subject stayed `string`. tRPC's OpenAPI generator
returns `raw` out of exactly the `||` chain at the top of this fixture.

The matching branch keeps the members the literal can inhabit and replaces a
member the literal is strictly narrower than, which is what turns `string` into
the literal. The complement drops only the members that *are* the literal — a
`string` that is not `"q"` is still a `string`, which `stillWide` pins.

`retarget` is the one intentional error, and it is the reason this fixture
exists as much as the rest: `target` is `"draft-07"` on both edges into the
second test, so the comparison has no overlap and `tsc` reports `TS2367` — which
surge only started reporting once the narrowing landed.
