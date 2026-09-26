# jsx-spread-attribute-checks

`createJsxAttributesTypeFromAttributesProperty` spreads the attributes left to
right: a spread of no object type is TS2698; under `strictNullChecks` an
attribute a later spread always writes is TS2783; and the relation reads the
property the spread wrote, so an overwritten value is related through the
spread's type. With no `ElementAttributesProperty` a class-like component's
props are its first constructor parameter, and no attributes object is
assignable to a primitive one (TS2322, TS2769 across overloads).
