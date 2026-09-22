# type-variable-relation-basic

tsc's relation for a generic body's own type parameters
(`structuredTypeRelatedToWorker` in relater.go): a type variable source
relates through its constraint, `unknown` when it has none; a type parameter
target admits nothing but itself, `never`, `any`, and a variable whose
constraint leads to it (`U extends T` is assignable to `T`, not the reverse).
`T & X` (here `NonNullable<T>` = `T & {}`) relates when some constituent does,
or through the constraint the constituents combine to. Anything is assignable
to `{} | null | undefined` (`isUnknownLikeUnionType`), which is what lets an
unconstrained `T` in, while `{} | null` still rejects it.

A generic class body sees its instance as `Box<T>`, so a member typed `T`
rejects a `string` write. A constraint surge cannot resolve (`K extends keyof
T`) must leave the variable permissive, not strict.
