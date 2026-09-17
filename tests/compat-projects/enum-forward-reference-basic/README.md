# enum-forward-reference-basic

Enum members are evaluated in order, so a member initializer naming a member
declared after it reads a binding that has no value yet (TS2651).

surge lowers an `enum` to type aliases while parsing, so there is no enum left
in the checker's AST — but this rule needs no types, only declaration order, so
it lives in the grammar pass alongside the other positional checks. An enum
member initializer must be a constant expression, which is why the walk has no
function bodies to stop at, unlike the class and module forward-reference walks.

Backward references are legal and are pinned as such, including inside an
arithmetic expression (`B = A | 1`) and for string enums. `const enum` is
reported the same way.

The related "members defined in other enums" half of tsc's message — a forward
reference across two declarations that merge into one enum — is not covered
here; only the single-declaration order is checked.
