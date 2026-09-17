# async-return-type-promise-basic

tsc's `checkReturnTypeAnnotation` requires an `async` function, method or
arrow to declare a `Promise<T>` return type (TS1064). surge accepted any
annotation. A written type reference is not reported, since an alias may name
`Promise`; an async generator returns `AsyncGenerator` and is exempt.
