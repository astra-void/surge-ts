# array-reduce-accumulator-basic

`Array.prototype.reduce` / `reduceRight` by the lib's three overloads: with no
initial value the accumulator is the element type; an initial value that fits
the element type keeps it (`reduce((a, x) => a + x, 0)`); anything else is the
generic overload, whose `U` is the explicit type argument or the initial
value's widened type (`""`, `new Map<…>()`, `{} as Record<…>`, `{ total: 0 }`).
surge left both the accumulator and the result `any`, so nothing downstream of
a `reduce` was checked and nothing inside the reducer saw the accumulator's
members.
