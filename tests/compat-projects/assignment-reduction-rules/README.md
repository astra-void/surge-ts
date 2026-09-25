# assignment-reduction-rules

An assignment narrows a union-declared binding to the members the value may be
assigned to (`getAssignmentReducedType`), and that relation includes tsc's
weak-type rule: an object whose properties are all optional accepts nothing
that shares none of them, so `Partial<B>` drops out when an `A` is written. A
mapped type applied to a union is that union (`Partial<A | B>`). A compound
assignment (`x += v`) instead leaves the binding at the base type of what it
held (`getTypeAtFlowAssignment`): an enum stays the enum, a possibly-undefined
string stays possibly undefined, and a literal widens.
