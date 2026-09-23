# computed-type-member-type-name-basic

A computed type-literal or interface member name that resolves only as a type
is TS2693; when it is a type literal's only property and the type is a union
of string- and number-like types, tsc suggests a mapped type (TS2690,
maybeMappedType).
