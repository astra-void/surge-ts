# generic-record-proxy-indexed-access-basic

A router/client proxy in miniature: a generic record is read back through
`TRouter['_def']['record']` and decorated by a mapped type that **references
itself** for nested records. tsc types `client.post.listPosts.query()` as
`Promise<Post[]>` and reports `TS2532` on `posts[0].title` under
`noUncheckedIndexedAccess`; surge reports nothing, because the whole proxy is
open.

The cause is not indexed access and not `noUncheckedIndexedAccess`, both of
which are already exact. It is that a *generic* alias which recurses into
itself resolves to the degradation sentinel: the resolution cycle is keyed on
the declaration alone, with no type arguments, so `Decorate<{post: …}>` and the
`Decorate<{listPosts: …}>` its own body asks for collide and the second is read
as a self-cycle. `resolve_type_alias` hands a generic back-edge
`Type::Unknown`, and every read downstream of it is silently accepted.

This fixture is **not registered as a swept preset**: it fails by default and
is closed only by `SURGE_GENERIC_RECURSIVE_ALIAS=1`, which discriminates
resolution frames by their resolved arguments. That gate is off by default
because it costs two false positives on the tanstack-query corpus (a recursive
tuple-prefix union whose recursive member collapses to `never`, narrowing the
type instead of leaving it open). Run it directly with

    pnpm run oracle:compare -- --project tests/compat-projects/generic-record-proxy-indexed-access-basic/tsconfig.json
