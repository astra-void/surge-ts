# tuple-identity-arity-basic

TS2403 asks every redeclaration of a `var` to be identical to the first, and
two tuple types are identical only as the same tuple target — the same arity,
the same element kinds (fixed or rest) in the same positions, the same
`readonly` flag — with identical elements. `ReadonlyArray<T>` is likewise a
different generic from `Array<T>`, whatever `T` is.

surge's identity check peeled the `readonly` wrapper away before comparing,
so `readonly [number]` and `[number]` (and `readonly number[]` and
`number[]`) passed as identical, and it had no case for a tuple with a rest
element: `[number, ...(string | boolean)[]]` was not identical to itself with
the union reordered.

Every TS2403 here is intentional and is a `tsc` error too; `libFrozen` and
the reordered `open` are not errors.
