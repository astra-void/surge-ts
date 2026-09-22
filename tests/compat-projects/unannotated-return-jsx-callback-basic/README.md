# unannotated-return-jsx-callback-basic

Two rules that decide whether a callback attribute in a *returned* JSX element
is an implicit `any`.

**An unannotated declaration's returns have no expectation.** Go checks a
return statement only against the annotation (`checkReturnStatement` via
`getReturnTypeFromAnnotation`, nil here). surge evaluated the returned
expression against the sentinel return type instead, and a sentinel expectation
suppressed implicit-any throughout it — so the callbacks in `ReturnsUnresolved`
and `ReturnsAny` were silent. tsc reports both: a component typed `any` or by
the error type gives its attributes no contextual signature
(`getContextualSignature` of `any` is undefined), so the parameter really is an
implicit `any`. This is tRPC's `next-sse-chat`, whose `Textarea` comes from an
unresolvable `~/components/input`.

**A component surge could not type is its own gap.** With that blanket
suppression gone, `ReturnsTypedProvider` would report `entries`: surge does not
infer the generic `makeProvider`'s return, so `stream.Provider` is its sentinel.
tsc types it and the callback is contextually typed. Only surge's own sentinel
(or a bare type parameter) marks a component's props unmodelled; `any` and the
error type do not, which is what keeps the first two reports. This is
tanstack-query's `ReactQueryStreamedHydration`.
