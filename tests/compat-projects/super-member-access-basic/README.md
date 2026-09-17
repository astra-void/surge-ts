# super-member-access-basic

`super.member` in a class member reads the base class: its instance in an
instance member, its constructor in a static one. surge lowered `super` to
nothing, so a missing member (TS2339), a bad argument (TS2345) or a
mismatched value read through `super` went unreported.
