# type-variable-narrowing-basic

Narrowing a generic body's type variable, as tsc does in flow.go:

- `typeof x === "tag"` (`narrowTypeByTypeFacts`): a variable whose constraint
  is already the tag's type stays itself, one whose constraint can never report
  the tag is `never`, anything else is `T & string` — still assignable to `T`
  and to `string`, and it reads the primitive's members.
- `instanceof` (`getNarrowedType` with `checkDerived`): a variable is derived
  from the class when its constraint is. The true branch keeps derived members
  and makes an underived variable `T & C` only when nothing else matched; the
  false branch drops derived members. A primitive or an anonymous object type
  never derives from a class (`isTypeDerivedFrom`), however little the class
  declares.
- a type predicate: a variable the predicate does not already cover is `T & P`.
