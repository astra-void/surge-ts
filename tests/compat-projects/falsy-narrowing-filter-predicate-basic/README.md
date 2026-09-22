# falsy-narrowing-filter-predicate-basic

The false branch of a truthiness test drops every member that is always
truthy (tsc's `getTypeWithFacts(type, TypeFacts.Falsy)`): an object or a
function cannot be falsy, so `if (!shape)` leaves only `undefined`, and a
lone always-truthy subject is `never`. `string` stays `string`, since `""`
is falsy.

`filter` infers a type predicate from an unannotated callback only when the
callback body is boolean-typed, and only when no true-branch member survives
the false branch. That covers `!!x` and `x !== undefined` over object and
function elements (zod's `errorMaps: [...].filter((x) => !!x)`). A body that
returns the element itself (`(x) => x`) is not a predicate.
