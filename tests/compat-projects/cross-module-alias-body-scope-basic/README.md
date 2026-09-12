# cross-module-alias-body-scope-basic

`lib.ts` declares `Record1<T>` and a `Caller<T>` alias whose body names it.
`consumer.ts` imports an interface returning `Caller<number>` and declares its
own two-parameter `Record1<A, B>`. Resolving the imported interface expanded
`Caller`'s body with the consumer's file-local declarations consulted first, so
`Record1<T>` bound to the consumer's declaration and reported a false TS2314 at
the library's span. tRPC's `router.ts` declares `DecorateRouterRecord` with one
parameter while `react-query` declares one with two.

The file-local table is now skipped whenever resolution has crossed into
another file's declaration scope, not only into a dependency's `.d.ts`; the
declaring file's own scope answers instead. The consumer's `Record1` is still
its own for its own uses, which `local` pins.
