# intersection-disjoint-property-basic

A property declared by two intersection operands has the intersection of their
types, and tsc's `getIntersectionTypeEx` reduces an intersection to `never` when
operands come from disjoint primitive domains (string-like, number-like,
void-like, …).

surge distributed `(string | undefined) & string` but kept each arm's first
operand, so `undefined & string` stayed `undefined` and the property read as
`string | undefined`. An arm of identical operands (`Date & Date`) was also
merged into a synthetic surface instead of collapsing to `Date`, and the
property read went silent.

A `this is { socket: Date }` predicate narrows the method's receiver, however it
is reached (`this.connection.isOpen()`), to the intersection of its declared
type and the predicate target, as tsc's `getNarrowedTypeWorker` does when the
two are unrelated. surge only narrowed a bare identifier receiver, and read an
unrelated predicate target as its degradation sentinel.
