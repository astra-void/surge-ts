# ambient-generator-basic

tsc's `checkGrammarForGenerator` reports TS1221 at the `*` of a generator
declared in an ambient context: a `declare function*`, a function inside a
`declare namespace` or `declare global`, and a method of a `declare class`.
Generators with bodies outside ambient contexts are fine.
