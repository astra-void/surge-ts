# tuple-target-structural-source-basic

An object type that is not itself an array or tuple reaches an array or tuple
target through tsc's structural comparison: the target is the object type its
apparent type is — `Array<T>`'s members over the element type, a tuple's
element properties by position and its literal `length`, and the number index
signature. So `interface StrNum extends Array<string | number>` with `0`, `1`
and `length: 2` is a `[string, number]` and a `(string | number)[]`, as
`RegExpMatchArray` is a `string[]`, and a tuple is a `StrNum`. A plain object
type with `0`, `1` and `length` lacks the array members; a tuple target with a
rest element takes no such source at all.

When exactly one member is missing tsc names it (TS2741, `Property '2' is
missing in type 'StrNum'`); with more, it keeps the plain TS2322 head for a
source that is not an array.

surge rejected every such source outright and reported the one missing
element as TS2322.

The errors from `missingThird` down are intentional and are `tsc` errors too.
