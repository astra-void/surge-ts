# var-redeclaration-types

What tsc's `checkVariableLikeDeclaration` compares when a `var` is declared
again: the symbol's type is its first declaration's declared type, not what an
initializer narrowed it to, and union members in any order are one type. The
rest element of an array pattern over a tuple is the sliced tuple
(`sliceTupleType`), and a `using` declaration is block-scoped, so two blocks
each declaring one are not a redeclaration.
