# grammar-missing-implementation-basic

An overload group with no implementation is `TS2391` (`TS2390` for
constructors), reported on the *last* signature of the group.

`Implemented`, `WithAbstract`, `Ambient` and `ambientFunction` pin the
exemptions: a group that has a body, an `abstract` member, a `declare class`
member, and an ambient function all report nothing. Only the "no
implementation anywhere in the container" case is checked — tsc's narrower
"implementation is not *immediately* following" half is deliberately not.
