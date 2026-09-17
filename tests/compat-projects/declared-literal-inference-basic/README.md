# declared-literal-inference-basic

Inference widened a literal type whenever it reached a type parameter through a
*nested* position — an array element, a tuple slot, an object member, or the
walk through a generic reference — because those recursion sites passed
`widen_literals: true` outright.

tsc decides this from the argument's **freshness**, not from where the parameter
sits: `inferTypes` widens a literal only when the source type is fresh, which is
what a literal *expression* produces. A declared type's literals survive however
deep the match goes. The trpc case was a `[ChunkIndex, 0, …] | [ChunkIndex, 1, …]`
stream matched against `AsyncIterable<TValue>`: `TValue` came back as
`[ChunkIndex, number, …]`, and assigning the result to the very type it came
from was then a false TS2322.

So the decision is taken once, from the argument expression, and carried through
every nested position. `take([1, 2, 3])` and `box({ v: 'a' })` still widen —
their literals are fresh — which is what the last two cases pin.

A constrained parameter keeps its literals whatever the argument is; that is
recorded once per call (`mark_keeps_literal`) and enforced where the candidate
is recorded, so it no longer depends on the position the match arrived through.
