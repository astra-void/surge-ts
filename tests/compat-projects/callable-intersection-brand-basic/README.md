# callable-intersection-brand-basic

`Base & { …all optional… }` is the brand idiom (`string & { _?: never }`), and
the intersection merge collapses it to the non-object side: an object surface
carrying only optional members says nothing about the value, and merging it
would keep `{ _?: never }` and reject `(string & brand) → string`.

That collapse is wrong when the surviving side is **callable**.
`ComponentType<P> & { getInitialProps?(…) }` — next's `AppType` — is a function
whose optional members are assigned and read like any other property, and
collapsing dropped them: `MyApp.getInitialProps = …` was a false TS2339 on a
type whose display still showed the member.

So a callable side goes through the merge and keeps the brand's members; a
primitive or a nominal reference still collapses. The reference half is
load-bearing in its own right: `WithRequired<T, K> = T & { [_ in K]: {} }`
(tanstack-query's) must answer `queryKey` from `T`'s own declaration rather than
from the brand's `{}`, or an incompatible `queryKey` is no longer rejected. The
`FetchQueryOptions` case pins that, the callable case pins the fix, and the
`string` case pins the idiom the rule was written for.
