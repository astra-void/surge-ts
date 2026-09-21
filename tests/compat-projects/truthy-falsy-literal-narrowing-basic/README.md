# truthy-falsy-literal-narrowing-basic

A truthiness test drops every member that is definitely falsy — the literals
`false`, `0` and `""` as well as the nullish ones. surge dropped only the
nullish ones, so `false | Perf` tested with `perf &&` kept `false` and every
member read was a false TS2339.

A `const` whose initializer is condition-shaped (`const perf = inBrowser &&
source`) is a condition alias, and `if (perf)` narrowed by the aliased
condition alone. It now narrows `perf` itself as well, in an `if`, a `while`,
an `&&` chain and a conditional. A classic alias (`const isText = typeof v ===
"string"`) still narrows `v` in both branches.
