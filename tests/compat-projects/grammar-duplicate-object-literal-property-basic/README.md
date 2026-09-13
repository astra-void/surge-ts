# grammar-duplicate-object-literal-property-basic

`TS1117` fires on every property after the first one of that name — including
the shorthand and the string/computed-literal spellings, which name the same
property.

`accessors` and `computed` pin the two non-reports: a `get`/`set` pair is one
property, and a computed key that is not a literal has no name to compare.
A duplicate written with *method* shorthand is `TS2300` in tsc, not `TS1117`,
so methods stay out of the check entirely.
