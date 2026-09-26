# enum-computed-member-initializers

tsc's `checkEnumMember` checks every member initializer as an expression, with
the enum's members in scope, and `computeConstantEnumMemberValue` requires one
that tsc's evaluator cannot give a value to be numeric (TS18033): a template or
concatenation over a `const` whose own initializer is not constant, a
destructured `const`, a `let` shadowing `Infinity`, a function, an assertion,
`undefined`, a `symbol`, the enum object itself — in a module, a block, a
function body and a namespace. Constant ones (`const` literals and sums, earlier
members, `A | B`, an import) are not reported. The evaluator also answers the
constant-only rules: a non-entity property access is computed (TS1066 in an
ambient enum, TS1061 after it), and the globals `NaN` and `Infinity` are values
a `const enum` rejects (TS2478, TS2477). surge checked none of the initializers.
