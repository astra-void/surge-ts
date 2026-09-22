# function-literal-alias-return-inference-basic

A function *literal* argument infers through a generic alias's body even when the
alias mentions the type parameter only in its return
(`Resolver<TOut, $Output> = (opts) => MaybePromise<DefaultValue<TOut, $Output>>`).
Go's `inferFromTypes` matches source and target by alias symbol and infers from
the type arguments directly; a literal's type carries no alias, so it walks the
signatures instead and the return reaches the naked `$Output`. surge admitted a
function argument into such a body only when the alias's *parameters* carried the
type parameters, so `run(() => 1)` inferred nothing and the result went silent.

The second case pins that a destructured callback parameter is bound in the
inference sketch too: without it the block body had no `ctx` and its return type
was unknowable.

Deliberately absent: the same call with a *declared* `Resolver` value still
infers nothing. That is the case the narrow gate protects — matching a declared
`TRPCLink<AnyRouter>` through its body bound tRPC's `TRouter` from a link
argument — and closing it needs the alias identity surge's resolved types do not
carry.
