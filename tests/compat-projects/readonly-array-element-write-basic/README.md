# readonly-array-element-write-basic

A write through an index into a readonly array is TS2542, reported on the whole
element access (`errorIfWritingToReadonlyIndex`); a readonly tuple's element is a
property, reported as TS2540 on the index. surge recognised only the
`readonly T[]` spelling, so the lib's `ReadonlyArray<T>` written by name — as a
parameter, or behind an initializer — was writable, and it anchored TS2542 on
the index. tsc prints `ReadonlyArray<T>` as `readonly T[]`.
