# class-heritage-type-argument-count

tsc's `resolveBaseTypesOfClass` treats a class's `extends` as an expression.
Only when the base constructor is itself a class is the clause read as a type
reference (`getTypeFromClassOrInterfaceReference`), with its argument-count
errors (TS2314/TS2707) on the clause. A base that is not a class — `Array`, a
`declare var Mup: MupConstructor`, an interface (TS2689) — derives from a
constructor function whose construct signatures take the type arguments, so
`class C extends Array {}` reports nothing about them. A type argument inside
the clause (`Box<G>`) is an ordinary type reference.
