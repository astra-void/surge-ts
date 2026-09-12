# possibly-undefined-receiver-basic

A non-optional member access, element access, or method call whose receiver can
be `undefined` is `TS18048` when tsc can name the receiver (`'box' is possibly
'undefined'`, dotted chains included) and `TS2532` otherwise (a call result, a
bracketed access). The access then continues on the defined part, so no
`TS2339` follows. Neither code existed in surge's catalog.

Two rules the fixture pins: `void` is not nullable to tsc (`nothing().toString()`
is a missing property, not a possibly-undefined receiver), and a later link of
an optional chain is judged without the chain's own short-circuit `undefined` —
`b?.value` is fine while `b?.inner.deep` with an optional `inner` still reports
`'b.inner'`. Guards, `!`, `?:` and `&&` keep the narrowed receiver silent.
