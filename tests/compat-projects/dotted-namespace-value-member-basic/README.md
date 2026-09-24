# dotted-namespace-value-member-basic

`namespace A.B {}` declares `A` with a member `B`. surge parses it as `A`
holding a namespace already named `A.B`; the value side must still key that
member `B` (not `A.B`), and the qualified names its members are published
under must be `A.B.member`. A member the namespace does not have is TS2339 on
the dotted namespace's own type.
