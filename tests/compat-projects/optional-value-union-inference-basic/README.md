# optional-value-union-inference-basic

`resolveValue<TValue>(value: undefined | TValue | ((q: Query) => TValue), …)` is
the shape TanStack Query's `resolveQueryValue` uses for every option that may be
a value or a callback. Called with `boolean | ((q: Query) => boolean) |
undefined`, tsc infers `TValue = boolean`.

surge zipped the two unions positionally whenever their arities matched, and
here they match by accident in the wrong order: the parameter reads `undefined |
TValue | ((q) => TValue)` while the argument reads `boolean | ((q) => boolean) |
undefined`, so `TValue` bound to the callback arm. The return type became
`((q) => boolean) | undefined`, which made every `!== false` on the result a
false TS2367 and the call itself a false TS2345 — four diagnostics here, and the
same four on `queryObserver.ts` in the corpus.

A union carries no member order, so a positional zip is only meaningful when
every member has a shape to pin it to. Once the expected union mixes a naked
type parameter with structured members, inference falls through to the
shape-matching path instead, and the `undefined` arm claims the argument's own
`undefined` so the naked parameter stands for the value alone.

tsc reports nothing here; the preset pins that surge agrees.
