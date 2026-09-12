# intersection-self-reference-cycle-basic

`Window & typeof globalThis` names itself through its own properties: `Window`
declares `window` and `self` as that intersection, and the global object
declares them as `Window`. surge merges an intersection eagerly into one object
surface, and a property declared by both operands is merged as its own
intersection, so the merge re-entered itself with the operands it was already
merging — `window` and `self` alternate with a period of two.

The plain annotation never reached the merge (its `typeof globalThis` operand is
still unresolved while the DOM globals are collected). The generic route did:
`Parameters<T>`/`ReturnType<T>` substituted into a contextual parameter type
resolve the intersection with the global object fully built. Unbounded, the
walk overflowed the stack; bounded by depth alone it fanned out exponentially
(two recursive properties per level, each level re-merging ~1000 members) and
peaked at 55 GB RSS on TanStack Query's `focusManager.test.tsx`.

The merge now detects a re-entry by operand identity and hands back the
enclosing merged surface, which is what tsc renders for `window.window` anyway.
tsc reports nothing here; the preset pins that surge terminates and agrees.

Not pinned here, because it predates this fixture: the DOM globals `window` and
`self` are resolved while `globalThis` does not yet have a value symbol, so
surge types them (and the `window`/`self` members read through an intersection)
as `Window` where tsc says `Window & typeof globalThis`. Assigning such a read
to a `Window & typeof globalThis` annotation is a false TS2322 (verified against
tsc 7.0.2 on 2026-09-10). The fixture reads through the chain and assigns to
`Window`, which both checkers accept.
