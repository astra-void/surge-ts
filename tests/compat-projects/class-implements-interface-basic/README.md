# class-implements-interface-basic

An `implements` clause contributes nothing to the class — it only constrains
it — so a class that does not declare a required member of an interface it
implements is `TS2420`, reported once per clause.

Only the *missing member* half of tsc's check runs: a member that is present
but whose type does not match is `TS2416`, which surge does not report yet.
The rest of the file is the non-reports — an optional member, an object-type
alias as the target, a member inherited from the base class, an accessor, and
an abstract class, which tsc checks exactly like a concrete one.
