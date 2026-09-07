# overload-merge-contextual-callback-basic

surge collapses an overload group into a single permissive signature rather
than resolving overloads, so a slot the overloads disagree on has to stay
usable for contextual typing.

Two things make that work. A slot whose overloads disagree widens to the
degradation sentinel rather than `any`: `any` claims the source wrote no
contextual type, so a callback nested in the argument became a false
implicit-any (`new ReadableStream({ start(controller) {} })`, whose three `new`
overloads disagree on the source object). And a slot that merges into a union
of several *callable* members of the same arity is surge's own artifact — tsc
picks an overload and never forms the union — so the arrow written there is
evaluated under a degraded expectation instead of being reported
(`addEventListener`, whose listener slot holds both the generic overload's
callback and `EventListenerOrEventListenerObject`).

The last function is the edge that keeps the second rule honest: a union of
callables with *different* arities is genuinely ambiguous, tsc refuses to type
the arrow there too, and the implicit-any is real. It is the single intentional
error.
