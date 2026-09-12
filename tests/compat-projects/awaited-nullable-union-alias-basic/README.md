# awaited-nullable-union-alias-basic

`await` is erased at parse time and `Promise<T>` is modelled as its awaited
`T`, so a promise whose awaited type can be `undefined` has to *collapse* when
it is resolved — a deferred `Promise<T>` reference stays opaque until peeled,
and every consumer that reads union members structurally (truthiness
narrowing, the `?.` short-circuit, the possibly-`undefined` check) misses the
`undefined` inside it.

The shape that hit this is a source type alias holding the promise as a union
member next to its own awaited type: TanStack Query's
`type Promisable<T> = T | Promise<T>` instantiated as
`Promisable<PersistedClient | undefined>`. Awaiting it left the union with a
`Promise<T>` member, `restored?.clientState.queries` reported
`'restored.clientState' is possibly 'undefined'` (eight times across the
persister tests), and `if (restored)` still typed `restored.timestamp` as
`number | undefined`.

The collapse is scoped to a promise resolved inside a *source* alias body whose
awaited type includes `undefined`. Both halves of that scope are measured, not
chosen: collapsing every nullable-awaited promise, or one inside a dependency's
own alias (`MaybePromise`, `Thenable`), shifted an unrelated zod assignability
verdict through the shared instantiation store; collapsing every promise at all
erased the `void | Promise<void>` and `return this.promise` distinctions the
deferred form keeps under the implicit-await model (ky, zod, trpc each +1).
