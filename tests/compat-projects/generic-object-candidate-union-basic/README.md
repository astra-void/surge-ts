# generic-object-candidate-union-basic

Two ways a type parameter's candidates were being lost.

**Later object candidates were dropped.** `record_type_argument_candidate` kept
the first candidate and, when a later one differed, only knew how to meet two
primitives at their base type; two object shapes fell through and the second was
discarded. `shallowEqual({ a: 1 }, { a: 2 })` bound `T` to `{ a: 1 }` from the
first argument alone and then checked the second against it — `Type '2' is not
assignable to type '1'`, and an excess-property error for `{ a: 1, b: 2 }`. tsc
infers the union of object candidates, so `T` is both shapes and neither call
reports. The union parameter `b: T | undefined` had a second hole in front of
it: the union arm gave up whenever an argument matched none of the *structured*
members (`undefined` here), and never handed the leftover to the naked one.

**Fresh object literals did not widen.** A fresh primitive literal already
widened for an unconstrained `T` (`subject(1)` is `{ next: (v: number) => … }`),
but an object literal's property literals did not, so `subject({ n: 1 })` bound
`T` to `{ n: 1 }` and `next({ n: 2 })` reported. Object and array literals now
widen the same way; a `const` assertion is not fresh and keeps its literals,
which `asserted` pins.

`shallowEqual` is query-core's; its own tests were two of the tanstack-query
aggregate's false positives.
