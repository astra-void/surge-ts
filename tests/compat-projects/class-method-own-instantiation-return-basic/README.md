# class-method-own-instantiation-return-basic

An unannotated method whose body returns `new` of its own class with the type
arguments written out (`context<T>() { return new Builder<T, TMeta>() }`) has
that instantiation as its return type (`getReturnTypeFromBody`). surge
synthesizes a class's instance members from syntax and typed such a method
`any`, so a builder chain lost every type at its first link — tRPC's
`initTRPC.context<Ctx>().create()`.

The three reports pin that the written type arguments, the enclosing class's own
parameters, and a genuinely missing member all come through the chain.
