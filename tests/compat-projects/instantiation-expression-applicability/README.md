# instantiation-expression-applicability

An instantiation expression (`f<T>` with no argument list, in a value or a
`typeof` query) whose type has no signature taking that many type arguments
(TS2635): an overload group, a value with no signatures at all, and a query
naming a function that takes more. Instantiations that fit stay silent.
