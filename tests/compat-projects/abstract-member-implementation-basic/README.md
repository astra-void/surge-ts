# abstract-member-implementation-basic

A class that is not itself abstract has to implement every abstract member it
inherits. tsc picks the code by how many are missing — one is `TS2515`, two to
five are listed as `TS2654`, six or more list the first four as `TS2655` — and
names the *direct* base class, type arguments included.

The negatives are the rest of the rule: `StillAbstract` is abstract itself,
`Implemented` inherits through a middle class that implements the member, and
`AllImplemented` covers the three shapes that count as an implementation — a
property, an accessor, and a constructor parameter property.
