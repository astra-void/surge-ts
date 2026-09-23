# readonly-array-mutable-target-basic

A `readonly` array or tuple is not assignable to a mutable array or tuple.
tsc's `reportErrorResults` runs `tryElaborateArrayLikeErrors` on the failed
pair first, which reports TS4104 (`The type '…' is 'readonly' and cannot be
assigned to the mutable type '…'`), and `reportRelationError` then drops its
own head — TS2322 in an assignment, return, initializer or property, TS2345 for
an argument — because that message names the same pair (relater.go). surge
chose TS4104 only for arguments and initializers, and only when the target was
written as an array or tuple type rather than an alias of one.

The relation itself also let a readonly source through a union target: after
trying each member against the readonly source it compared the members again
with the mutable shape the source resolves to, so `readonly string[]` was
assignable to `string[] | undefined`. A target with more than one non-nullable
member keeps TS2322 with the readonly message beneath it (`either`).

`elements.ts` pins writes and `delete`s through an index: a readonly tuple's
fixed elements — an open tuple's leading ones included — are properties
(TS2540, and TS2704 under `delete`), and every other index lands on the
read-only index signature (TS2542 on the whole access, `isDeleteTarget` too).
