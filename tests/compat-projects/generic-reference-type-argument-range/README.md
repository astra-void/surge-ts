# generic-reference-type-argument-range

A generic type whose trailing type parameters have defaults takes between
`getMinTypeArgumentCount` (the parameters up to the last one without a default)
and all of them. tsc reports a reference outside that range as TS2707
("requires between 2 and 3 type arguments"), and uses TS2314 only when every
parameter is required. surge always reported TS2314 with the full count.
