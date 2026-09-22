# index-signature-property-constraint-basic

tsc's `checkIndexConstraints` (checker.go): each property of an interface or
class with an index signature must be assignable to every index signature
that applies to its name — a string index to all of them, a number index to
the numerically named ones (TS2411) — and a number index to the string index
(TS2413). surge emitted neither.

The anchor follows `checkIndexConstraintForProperty`: the property when the
declaration declares it, else the index signature when it declares that, else
(an interface whose bases bring them separately) the interface name. A base
that holds both reports the conflict itself, and a member or index that comes
from another fragment of a merged declaration (a global `interface Object`
augmentation meeting the lib's) is reported there, not here.

A generic declaration is checked as itself over its own type parameters, so an
unconstrained `T` property conflicts with a `string` index.
