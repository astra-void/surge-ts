# class-field-initializer-type-basic

An unannotated class property takes the widened type of its initializer
(`getWidenedTypeForVariableLikeDeclaration`); a `readonly` one keeps the
literal. surge typed every unannotated property — instance or static — as
`any`, so nothing read from or written to such a field was checked.

The instance side is assembled from written types before expressions are
checked, so the lowering covers initializers whose type is evident from their
syntax: primitive literals (including a negative number and a template with no
substitutions), arrays of one primitive literal kind, and object literals of
those. An object literal's members widen even when the property is `readonly`,
because each member is itself a mutable location. An empty array literal is
`never[]`, so pushing onto it is an error. Other initializers (`new X()`, a
call) still leave the property `any`, as does an object literal with a
spread or computed key; a member surge cannot lower is `any` on its own.
