# override-and-modifier-order

`override` is checked against the base class's type, as tsc's
`checkMembersForOverrideModifier` does: a member the base type lacks —
including the lib `Object` and `Function` members every object and
constructor has — cannot be `override` (TS4113, or TS4117 with a spelling
suggestion; TS4112 without a base class), and under `noImplicitOverride` one
it has must be (TS4114, TS4115 on a parameter property, TS4116 over an
abstract base method). Static members are looked up on the base constructor.
A computed name that is neither a literal nor a unique symbol names no member
(TS4127). In a JavaScript file the `@override` tag is the modifier and the
messages name it (TS4119–TS4123).

Modifier order (`checkGrammarModifiers`) holds for parameter properties,
type parameters (`in` before `out`), module declarations and decorated
declarations, while a JSDoc modifier tag has no written order to break. Any
method named `constructor` is a constructor, so `static` on one is TS1089.
