# heritage-cycles-and-conflicts

A class whose `extends` chain leads back to itself cannot resolve its base
constructor (TS2506 at each class on the cycle), and an interface whose bases
lead back to it cannot resolve its base types (TS2310, generic ones displayed
with their parameters); a class named in `extends` before its declaration is
used too early (TS2449). An interface whose bases supply the same member
differently cannot extend both (TS2320), and only then is each base's member
checked against the interface's own — through the interface's type parameters
as real type variables, so `item: { wrapped: T }` and `a: () => T` fail against
`item: T` and `a: <T>() => T` (TS2430).
