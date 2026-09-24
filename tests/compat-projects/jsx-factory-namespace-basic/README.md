# jsx-factory-namespace-basic

tsc looks up `IntrinsicElements` and the other JSX members in the namespace
`getJsxNamespaceAt` picks: the `JSX` member of the namespace the tag's factory
is rooted at (the file's `@jsx` pragma, else `jsxFactory`, else
`reactNamespace`, else `React`), and the global `JSX` only when that namespace
has no `JSX`. `pragma.tsx` compiles to `dom`, so `<p>` is typed from
`dom.JSX.IntrinsicElements`; `fallback.tsx` compiles to `plain`, which has no
`JSX`, so it reads the global one.

tsc resolves a closing tag again (`checkJsxElementDeferred`), so an unknown
intrinsic tag or an unresolved component is reported at both tags. A tag that
is `this` or starts with `this.` is a value, not an intrinsic element.
