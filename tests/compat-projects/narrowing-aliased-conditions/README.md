# narrowing-aliased-conditions

tsc narrows by an aliased condition (`narrowType`, flow.go): testing an
unannotated `const` declared on its own inlines its initializer, an alias
within it is inlined in turn up to five levels (`inlineLevel`), and the
inlined condition narrows a reference only if `isConstantReference` accepts
it — `this`, a `const`, or a parameter or `let` never assigned in its
function, reached through `readonly` properties or a `readonly` tuple's
element. A `var` never qualifies.

- `nested.ts`: `isString || isNumber` behind another alias narrows (tsc's
  `controlFlowAliasing` f13); five levels narrow, six do not.
- `predicate.ts`: a type-predicate call is an alias too; a mutable property
  argument does not narrow through it.
- `discriminant.ts`: a destructured or aliased discriminant narrows the object
  it came from in a `switch` and in the operand of `&&`
  (`getCandidateDiscriminantPropertyAccess`).
- `annotated.ts`: an annotated alias, or a `let`, inlines nothing.
- `constant.ts`: a reassigned parameter, a mutable property, an assignment
  after the test, a reassigned tuple and a `var` do not narrow through the
  alias; a readonly property, a readonly tuple element and a `let` never
  assigned do.

Every error here is also tsc's.
