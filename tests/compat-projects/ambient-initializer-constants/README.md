# ambient-initializer-constants

tsc's `checkAmbientInitializer`: in an ambient context (a `declare`d
declaration, a declaration inside one, or a `declare` class field) only a
`const` — or a `readonly` property — without a type annotation may have an
initializer, and it must be a string, numeric, bigint or boolean literal
(numeric and bigint possibly negated) or a literal enum reference (TS1254);
every other initializer is TS1039.
