# method-return-inferred

An unannotated class method's return type as tsc's `getReturnTypeFromBody`
gives it: every `return` unioned with subtype reduction, `undefined` for an end
the body can reach, a fresh unit literal widened while a union of literals or an
asserted literal stays, and `this` bound to the instance (or, in a static
method, the class). A mutable binding of such a call widens the fresh literals;
a script's own classes are visible to the bodies.
