# modifier-order-basic

tsc's `checkGrammarModifiers` requires class member modifiers in a fixed order:
an accessibility modifier before `static`, `override`, `readonly`, `async` and
(in an abstract class) `abstract`; `static` before `override`, `readonly` and
`async`; `override` before `readonly` and `async`. The first violation of a
member is TS1029 on the modifier written too late. surge's AST keeps which
modifiers a member has but not their order, so none was reported; the grammar
pass now reads the order from the source, past decorators and comments.
Correctly ordered members are pinned as clean.
