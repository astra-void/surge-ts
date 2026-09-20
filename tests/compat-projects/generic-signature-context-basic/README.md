# generic-signature-context-basic

`instantiateSignatureInContextOf`: a generic signature compared with a
non-generic one is first instantiated with what that signature's
parameters infer for its type parameters, so `<T>(x: T) => T[]` against
`(x: number) => string[]` compares `number[]` with `string[]`. The same
instantiation lets a generic function passed as an argument contribute
its return to the call's inference
(`instantiateTypeWithSingleGenericCallSignature`). surge's type-parameter
placeholders related like `unknown`, which accepted every such pair and
inferred nothing from the argument. An annotated binding keeps its
declared type through a rejected write.
