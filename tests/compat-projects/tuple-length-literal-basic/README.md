# tuple-length-literal-basic

`createTupleTargetType` gives a tuple without a rest element a `length` of
its arity as a number literal — the union of the lengths from `minLength` up
when trailing elements are optional — and names each element as a property by
its position. A rest element makes `length` plain `number` and ends the named
elements.

surge typed every tuple's `length` as `number`, so a literal annotation
(`const n: 1 = single.length`) was a false TS2322 while a comparison against
a length the tuple cannot have (`pair.length === 3`, TS2367; `case 2` on a
one-element tuple, TS2678) went unreported; and a tuple had no `"0"`
property, so `[string]` against `{ 0: number }` read as a missing property
(TS2741) instead of an incompatible one (TS2322).

The errors from `tooLong` down are intentional and are `tsc` errors too.
