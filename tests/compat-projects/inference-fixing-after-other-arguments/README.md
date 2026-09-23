# inference-fixing-after-other-arguments

tsc's `inferTypeArguments` infers from every argument that is not context
sensitive first, then from the context-sensitive ones left to right; typing a
callback's unannotated parameter fixes the type parameters its contextual type
names at the inference made so far.

- `store.subscribe((state) => state.count, (count) => …)` and
  `watch(counter, (s) => s.count, (c) => …)`: the selector's return gives
  `U = number` before the listener's parameter fixes it.
- `mapObject(rec, (s) => s.length)`: `rec` gives `T = string` first.
- `both((x) => 1, (x) => "")`: the first callback fixes `T` before the second
  is inferred from; with no candidate it is fixed at `unknown`, so the second
  callback's `""` is no error.

surge's walk does not always record the candidate an earlier argument gives
(an index-signature parameter, a signature it could not read), so a type
parameter an earlier argument's parameter names is left unfixed rather than
fixed at `unknown`. The intentional error, `toUpperCase` on the inferred
`number`, is a tsc error too.
