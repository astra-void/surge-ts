# loose-null-widening

Without strictNullChecks tsc widens `null` and `undefined` to `any` wherever
an inferred type holds them (`getWidenedType`): a getter returning `null`, a
property initialized with `null`, `[null]` or `[]`, and an array or object
literal of them in a variable declaration — a union absorbs them. Every
write here is therefore fine.
