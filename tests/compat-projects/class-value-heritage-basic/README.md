# class-value-heritage-basic

A class's `extends` clause is an expression: tsc's
`getBaseConstructorTypeOfClass` checks it as a value and takes the base
instance from its construct signatures, so `extends Ctor` is fine when `Ctor`
is a `const` holding a constructor and names no type at all. surge builds a
class's value while collecting signatures, before the file's `const`s are
bound, so such a base missed both lookups and was reported as TS2304 — twice,
the second time with no location, because the member-compatibility check
resolved the name again outside the heritage position. A base that names
neither a type nor a value is still TS2304, once, on the name; so is an
interface base that names no type.
