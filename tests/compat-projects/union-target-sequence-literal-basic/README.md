# union-target-sequence-literal-basic

An array literal against a union target took the union's *lone* array-or-tuple
member as its contextual type and gave up when there were several, so the
literal was evaluated context-free and widened: `['-', 2]` against
`['+', number, number] | ['-', number] | ['++', number]` became
`(string | number)[]` and was then rejected. ts-pattern's tuple tests are
written that way throughout, and so is `A[] | B[]`.

The literal itself says which member it is, the way a discriminated object
literal does. Candidates are the tuples of the same arity plus every array
member; a candidate fits when each written element is assignable to its slot.
Element types are read **unwidened** — the literal `'-'` is the whole
discriminator between `['-', number]` and `['++', number]`, and `'b'` between
`A[]` and `B[]`. Exactly one fitting candidate wins; anything else keeps the
context-free behavior, so an ambiguous literal is never typed against a guess.

`nestedArrayMember` covers the array reached through an object literal's
property, which is where ts-pattern's own cases sit.

A wrong element (`run(['-', 'two'])`) is deliberately not pinned here: tsc
reports `TS2322` on the offending element and surge `TS2345` on the whole
argument, a reporting-granularity difference that predates this fixture and
would pin the wrong thing.
