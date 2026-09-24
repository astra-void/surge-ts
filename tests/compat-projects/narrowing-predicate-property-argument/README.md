# narrowing-predicate-property-argument

A type predicate narrows the argument it tests, and tsc infers a generic
predicate's type parameters from that argument (`getEffectsSignature`
resolves the call, `getTypePredicateArgument` picks the reference). When the
argument is a property (`isDefined(this.testNumber)`, `isDefined(box.inner.value)`),
the type to infer from is the property's own. surge inferred from the root
binding's type or not at all, so `value is NonNullable<T>` never resolved for
a property and the guard narrowed nothing.

A predicate over a property also narrows only that property: `isString(o.p)`
says nothing about `o` itself (`notTheBase`). The one error, in the last
branch of `Holder.foo`, is tsc's too: `isDefined(box.inner)` does not narrow
`box.inner.value`.
