# template-literal-infer-basic

A conditional whose `extends` clause is a template literal with `infer`
placeholders binds each capture from what the check type's text leaves between
the template's fixed texts (tsc's `inferToTemplateLiteralType` over
`inferFromLiteralPartsToTemplateLiteral`). An `infer X extends C` capture
converts to the literal the constraint prefers (`"100"` → `100`, `"true"` →
`true`) or falls back to the constraint when it is not a round-trip literal,
and the branch is the check type's assignability to the template instantiated
with the captures.

surge dropped the `extends C` constraint at parse time and never bound the
captures: the pattern resolved to `string`, took the true branch for any
string, and every capture in it was a false TS2304.
