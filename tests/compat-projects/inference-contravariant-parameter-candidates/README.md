# inference-contravariant-parameter-candidates

tsc's `inferFromSignature` infers a callback argument's parameter types
contravariantly (`inferFromContravariantTypes`), into a type parameter's
`contraCandidates`, and `getInferredType` combines the two lists:

- `getContravariantInference` takes the common subtype of the contravariant
  candidates, so `on((e: Base) => {}, (e: Derived) => {})` infers `Derived`
  and both handlers fit.
- With both kinds, the covariant inference is preferred when it is neither
  `never` nor `any` and is assignable to some contravariant candidate:
  `emit(derived, (e: Base) => {})` infers `Derived`, and
  `emit(base, (e: Derived) => {})` infers `Derived` and rejects `base`.
- A method's signature is bivariant, so its parameters infer as ordinary
  candidates: `withMethod(derived, { handle(e: Base) {} })` infers `Base`.

surge recorded every position as a covariant candidate and combined them as
one list. The two intentional errors — `base` against `Derived`, and `m1.b`
on the inferred `Base` — are tsc errors too.
