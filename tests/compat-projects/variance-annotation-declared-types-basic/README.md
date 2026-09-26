# variance-annotation-declared-types-basic

A type alias takes `in`/`out` annotations only when its declared type is an
anonymous object, function, constructor or mapped type (TS2637). `keyof`, a
generic indexed access or conditional type, and a reference to an interface, a
class, or an alias that resolves to one of those or to a type parameter are
none of them; an alias to an object literal type, an indexed access that
resolves to one, and a union whose `never` member drops away are. `in`/`out`
on a class member or a function's type parameter is TS1274 at each site.
