# property-truthiness-discriminant-basic

`if (opts.ssr)` over a union whose members declare `ssr: true | (() => …)` and
`ssr?: false` narrows to the first member, and the `else` branch to the second.
surge only decided a truthiness test on a property when the leaf was a single
unit type — a `true | fn` leaf and an optional `false` leaf both stayed
undecided, so nothing was dropped and `opts.ssrPrepass` was a false TS2339.
tRPC's `withTRPC` wraps `WithTRPCSSROptions | WithTRPCNoSSROptions` exactly so.

A leaf now decides by tsc's type facts: a callable, an array, or a shape with at
least one member is always truthy, and a union decides when every member agrees.
Below an optional property only a falsy leaf still decides, since the property
may be absent. `{}` admits `""` and `0`, so a memberless object stays undecided,
which `emptyStaysWide` pins.
