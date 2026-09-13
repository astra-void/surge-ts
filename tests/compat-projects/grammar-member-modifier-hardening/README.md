# grammar-member-modifier-hardening

The member-level grammar rules, all of them decidable from the syntax alone:

- an initializer in an ambient context — a `declare const`, a `declare
  namespace` member, a `declare class` property (`TS1039`),
- `abstract` on a member of a class that is not abstract (`TS1244` for a
  method, `TS1253` for a property),
- a `set` accessor with a return type (`TS1095`) or with a parameter list that
  is not exactly one (`TS1049`),
- one name written in an object literal as both an accessor and a plain
  property (`TS1119`), reported on whichever came second.

`Proper` is the same class body written legally, and pins that none of the
member rules fire on an abstract class.
