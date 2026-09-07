# instantiation-expression-basic

`make<string>` with no argument list is an instantiation expression (TS 4.7),
not a call. surge lowered it to a zero-argument call and reported `TS2554` on
every one — tRPC's `const createClient = createTRPCClient<TRouter>` is exactly
this shape.

The instantiation itself is still not modelled: the reference keeps the generic
type and a later call infers from its own arguments, which is why
`makeStrings('a')` types. The two intentional errors pin the parts that must
keep reporting — a genuinely wrong element type, and a real zero-argument call
of a one-parameter signature.
