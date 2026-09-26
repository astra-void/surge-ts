# exact-optional-property-types-basic

tsc's `exactOptionalPropertyTypes`. An optional property declared without
`undefined` reads as its type plus a *missing* type: `undefined` is not
assignable to it once it is written (TS2412 on a dotted write), and a value
that may be `undefined` makes the whole assignment or argument fail, headed
TS2375 or TS2379. A property that spells out `| undefined` is unaffected.
`"p" in o` and `o.hasOwnProperty("p")` narrow the read of `o.p`, and `delete`
requires the optional modifier itself (TS2790).
