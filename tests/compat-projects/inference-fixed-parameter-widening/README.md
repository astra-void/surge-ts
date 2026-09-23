# inference-fixed-parameter-widening

Two steps of tsc's inference for callback arguments:

- The fixing mapper. A callback parameter written without an annotation is
  typed from the contextual signature (`assignContextualParameterTypes`), which
  fixes every type parameter its contextual type names at the inference made
  so far; a fixed inference widens its literal candidates (`widenLiteralTypes`
  holds once `isFixed` is set), and no later candidate changes it. So
  `g8(1, x => x + 1)` infers `number` although `T` stands at the top level of
  the return type, and `foldLeft(true, (acc, t) => acc && t)` infers `boolean`.
- `applyToParameterTypes`: a rest parameter of the target signature infers from
  the source's parameters from its position on, as one tuple
  (`getRestTypeAtPosition`), so `callr(sn, f15)` binds `T` to
  `[string, number]`.

The two intentional errors assign the widened results to `string` and `"a"`
and are tsc errors too.
