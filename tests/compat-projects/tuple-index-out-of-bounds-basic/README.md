# tuple-index-out-of-bounds-basic

tsc's `getPropertyTypeForIndexType`: a literal index past the end of a fixed
tuple is TS2493 ("Tuple type … has no element at index …"), a negative one is
TS2514, and the read is `undefined`. surge reported TS2339 for an identifier
receiver and nothing for any other receiver or for a negative index.

Array destructuring reads each element the same way, and tsc anchors the error
on the binding element. A binding with a default reads with
`AccessFlagsAllowMissing`, so a tuple too short for it is not an error — that
case is pinned as clean.
