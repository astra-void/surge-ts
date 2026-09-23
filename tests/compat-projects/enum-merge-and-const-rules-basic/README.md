# enum-merge-and-const-rules-basic

Across an enum's declarations only one may leave its first member uninitialized (TS2432). A const enum member evaluating to an infinite value is TS2477 and to NaN is TS2478. A const enum object may appear only as the object of a property or element access, in a `typeof` query, or in an export — TS2475 elsewhere — and an element access on it needs a string-literal index (TS2476).
