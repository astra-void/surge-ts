# object-literal-predicate-members

A method or arrow member of an object literal whose written return type is a
type predicate narrows its argument when called through the object
(`Utils.isNamed(node)`), as a declared method does: the member's function
type keeps its written signature (`DeclaredMemberSignature`), which is where
a guard reads the predicate. Surge dropped it from the literal's member, so
`Utils.isNamed(node) && node.name` reported the unnarrowed union — the
typescript-eslint `ASTUtils` pattern in tanstack's eslint plugin.
