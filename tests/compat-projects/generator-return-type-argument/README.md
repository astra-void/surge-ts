# generator-return-type-argument

tsc's `unwrapReturnType` relates a generator's `return` to the `TReturn`
argument of the generator type it is declared with
(`getIterationTypeOfGeneratorFunctionReturnType`), `any` when left to its
default, and an async generator awaits the returned value first. The object
literal is not checked against `Generator` itself.

That relation needs a return type annotation (`getReturnTypeFromAnnotation`).
A contextually typed generator expression is related through its whole
inferred signature instead, so `strategy`'s argument is TS2345 at the call
rather than a mismatch at its `return`.
