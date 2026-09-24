# template-literal-constant-evaluation

tsc's `checkTemplateExpression` first runs the constant evaluator over an
untagged template (`evaluateTemplateExpression`, `evaluateEntity`): string
and numeric literals, an enum member, a `const` whose initializer evaluates,
and a nested template all have values, and a template made only of those is
the string literal they spell. surge evaluated literal substitutions only, so
`` `${AnimalType.cat}` `` was a `string`, and a `case` written that way
(tsc's `discriminatedUnionTypes4`) narrowed nothing.

The two errors — a `let` widens the literal, and a template over a `string`
is not evaluated — are also tsc's.
