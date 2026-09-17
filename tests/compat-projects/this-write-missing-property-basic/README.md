# this-write-missing-property-basic

In a `.ts` file `this.x = …` never declares `x` (only JavaScript binds it
that way), so a write to a member the class does not declare is TS2339 —
in a constructor, a method, a subclass method, and on `typeof C` in a static
method. surge checked such a write only when the member existed.

Reporting it exposed that a class's own index signatures were dropped by the
parser, so any class with `[key: string]: T` had every dynamic member reported
missing — on reads already, and now on writes. Class index signatures now reach
the instance type like an interface's. Only `string` and `number` keys are
kept: a `symbol` key (Prisma's generated client declares one) answers no named
member, which the last case pins.
