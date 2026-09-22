# assertion-contextual-object-literal-basic

The operand of `x as T` and `<T>x` takes `T` as its contextual type (Go's
`getContextualType`, checker.go:29718), so the methods and arrow members of an
asserted object literal get their parameters typed from `T` and are not
implicit `any`. surge passed the asserted type down only when the operand was
itself an arrow function; an object literal was evaluated with no context, and
every parameter in it was a false TS7006.

An assertion relates by comparability, not assignability, so the context is all
it contributes: a missing member (`{} as Ctx`) and an extra one are not errors.
The last case pins that the context stops at the literal's own relation — a
genuine mismatch *inside* a method body is still reported.

Not covered here, and still divergent: a method whose body returns a type its
contextual return rejects (surge checks that return against the context; tsc
leaves it to the assertion's comparability), a nested array literal whose
element the context rejects, and `satisfies`, whose operand tsc types the same
way.
