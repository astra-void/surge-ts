# ambient-module-precedes-package-basic

Go's `resolveExternalModule` (checker.go:15369) calls `tryFindAmbientModule`
*before* any file resolution, so a script-level `declare module "mypkg"` wins
over a package of that name whatever the package's entry point looks like.
surge yielded to the ambient block only when the resolved file was itself a
script, so a package whose entry is a module took precedence.

That is not hypothetical: tRPC installs the userland `querystring` polyfill,
whose `index.d.ts` imports from `./decode` and is therefore a module. Every
type in `@types/node`'s `declare module "querystring"` — `ParsedUrlQuery` above
all — then read as "has no exported member", so `NextRouter.query` and
`GetStaticPropsContext.params` degraded and the `TS4111` reports that depend on
their index signature disappeared.

`parse` is imported as a value as well: the ambient block supplies both, and
binding it to the package instead is what made the divergence silent rather
than an unresolved-name error.
