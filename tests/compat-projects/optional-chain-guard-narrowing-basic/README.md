# optional-chain-guard-narrowing-basic

A truthy test of `opts?.transformer` proves two things at once: the property is
not `undefined`, and `opts` itself is not nullish — a nullish base makes the
whole chain `undefined`, which is falsy. surge narrowed only the property, so
the `undefined` member of `opts` survived and the guarded read still came out
`string | undefined`. tRPC's hey-api client config is this exact ternary.

`elseBranchProvesNothing` is the intentional error: the guard's *false* branch
proves neither half, so the read there is still `string | undefined`.
