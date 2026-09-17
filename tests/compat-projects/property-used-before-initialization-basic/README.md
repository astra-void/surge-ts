# property-used-before-initialization-basic

Field initializers run in declaration order, before the constructor body, so an
initializer may only read a property whose own initializer has already run
(TS2729). Nothing reported it.

Two parts of `checkPropertyNotUsedBeforeDeclaration` are easy to miss, and both
are pinned because getting either wrong changes the verdict on ordinary code:

- **A property with no initializer never counts as "declared before use"
  through `this`, even when it appears earlier in the class.** That is the
  `declaration.Initializer() == nil` clause of
  `isBlockScopedNameDeclaredBeforeUse`, and it is why `ReadsUninitializedProperty`
  is an error despite `a` being declared above `b`: assigning `a` in the
  constructor happens *after* `b`'s initializer runs. Only a `!` definite
  assignment exempts it.
- **An optional property is exempt outright** (`isOptionalPropertyDeclaration`)
  — reading it as `undefined` is what its type already says.

A static property is reported the same way, through `ClassName.x` rather than
`this.x`. A method is not a property declaration and is always available. An
arrow defers its body, including one nested in an object literal, so an early
read inside one is legal.
