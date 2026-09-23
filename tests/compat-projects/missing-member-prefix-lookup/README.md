# missing-member-prefix-lookup

tsc's `checkAndReportErrorForMissingPrefix` turns an unresolved name inside a
class into "did you mean the static member `C.x`" (TS2662) when the class's
constructor type has a property of that name, and into "did you mean the
instance member `this.x`" (TS2663) when its `this` type has one. Both lookups
are `getPropertyOfType`: declared and inherited members plus what the apparent
type adds (`Function`'s members on the constructor type, `Object`'s on both),
never an index signature. A generic class answers from its written members like
any other class.
