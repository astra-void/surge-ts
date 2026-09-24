# inference-object-literal-candidate-union

tsc's `getCovariantInference` combines every candidate a call recorded for a
type parameter:

- `unionObjectAndArrayLiteralCandidates`: when there are several candidates,
  all object and array literal types are replaced by their one union, placed
  after the other candidates.
- `getCommonSupertype`: under `strictNullChecks` `null` and `undefined` are set
  aside, the leftmost candidate no later one is a supertype of is chosen
  (`isTypeSubtypeOf`, under which a source lacks no target property, optional or
  not, and an object literal target has every property the source has), and the
  nullable members are added back.
- `getWidenedType`: object literals widened together are normalized, each
  gaining the properties its siblings write as optional `undefined` members.

So `each3(undefined, { x: 6, z: 1 }, { x: 6, y: "" })` infers the normalized
union and the third argument is no excess-property error, while
`orElse(maybeNamed, { id: 2, name: "b" })` infers `Named` (the literal lacks
`tags`, so `Named` is not its subtype). The four intentional errors — the
unrelated `other`, and the literals carrying `extra` or `other` checked against
the inferred `Named` — are tsc errors too.
