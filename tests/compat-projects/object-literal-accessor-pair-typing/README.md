# object-literal-accessor-pair-typing

Object-literal `get`/`set` pairs typed as tsc types them:

- `getTypeForVariableLikeDeclaration`: a set accessor's parameter written
  without a type takes the return type of the getter of the same name —
  annotated or inferred from its body, in either declaration order. The pair is
  matched by property name, so `get 'size'()` pairs with `set size(v)` and
  `get 0x10()` with `set 16(v)`.
- `getTypeOfAccessors` / `getReturnTypeFromAnnotation`: the property is the
  getter's annotation, else the setter's parameter annotation, else what the
  getter's body returns; an unannotated getter's `return` statements are
  checked against the setter's annotation.
- `isReadonlySymbol`: a getter without a setter is a read-only property, so
  writing it is TS2540 and the literal's type is `{ readonly id: number }`.

The intentional errors — `mismatch` (the setter's `value` is `string`), the
write to `getOnly.id`, and `fromSetter`'s `return "text"` against the setter's
`number` — are all tsc errors too.
