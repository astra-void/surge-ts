# inherited-abstract-generic-heritage

tsc's `checkKindsOfPropertyMemberOverrides` reports an abstract member a
non-abstract class inherits without implementing (TS2515, TS2654/TS2655 for
several) against the class's declared type and its base type:
`typeToString` of each, so a generic class reads `C<T>` and its base reads as
the heritage clause instantiates it under the class's own type parameters
(`A<T>`, `A<Map<K, V>>`). Those type arguments are in scope there; resolving
them outside the class reported a false TS2304 for `T`.

`B` is abstract and `G` implements both members, so neither is an error.
