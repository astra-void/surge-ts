# class-computed-member-names-basic

A class member's computed key is an expression surge had never checked: its
type must be `string`, `number`, `symbol` or `any` (TS2464, tsc's
`checkComputedPropertyName`), and its names resolve like any others. A class
property's computed name that is neither a literal nor an entity name cannot
be bound (TS1166); the same rule is TS1169 in an interface and TS1170 in a
type literal (`checkGrammarForInvalidDynamicName`).
