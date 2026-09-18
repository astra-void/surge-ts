# member-accessibility-basic

tsc's `checkPropertyAccessibility`: a `private` member is reachable only inside
the class that declares it (TS2341), a `protected` one inside that class or a
class deriving from it (TS2445). The test is lexical, so a nested function or
class inside the body still reaches the members, and `c["x"]` is the
deliberate escape hatch. surge modelled neither modifier and reported none of
it.
