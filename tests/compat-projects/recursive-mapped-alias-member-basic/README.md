# recursive-mapped-alias-member-basic

A generic alias that re-enters itself through a mapped type's member
(`Decorate<R[K]>`, tRPC's `DecoratedProcedureRecord<TRoot, $Value>`) is legal
recursion that tsc instantiates on demand. surge treated the re-entry as a cycle
and read the member as its degradation sentinel, so `client.post.list.query()`
was never checked.

The fix made three older gaps reachable, each covered here:

- each `unique symbol` is its own type, so `$input extends $output` is false;
- the lazy member's arguments are resolved at the back-edge, and the
  instantiation is never interned as a reference to itself;
- truthiness and `typeof` narrowing look through a reference that resolves to
  a union (`meta.id` is `Replace<string | undefined, …>`).
