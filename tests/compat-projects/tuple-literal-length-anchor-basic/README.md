# tuple-literal-length-anchor-basic

An array literal that is longer or shorter than the tuple it is checked
against has no element at fault, so tsc's `elaborateArrayLiteral` finds nothing
to descend into and the failure is the whole value's. It is reported where any
other unassignable value would be: the declared name, the assignment target,
the argument (as TS2345, not TS2322), the `return`, the property name or the
enclosing array's element. surge reported it inline on the first excess
element — always as TS2322 — and printed a too-short literal's source type as
`unknown[]`. A wrong *element* is still reported on that element.
