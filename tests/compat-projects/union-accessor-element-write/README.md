# union-accessor-element-write

Writing a literal key through a union receiver checks the value against tsc's
synthetic union property write type — the union of each constituent's write
type, a setter's parameter type for an accessor — not against the getters'
read type, so `42` and `true` fit while `null` does not.
