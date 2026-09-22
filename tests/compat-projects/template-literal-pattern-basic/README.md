# template-literal-pattern-basic

Template literal types, as tsc's `getTemplateLiteralType` builds them and
`isTypeMatchedByTemplateLiteralType` relates them: a string literal
matches by the pattern's fixed texts and each placeholder's kind
(`${number}` takes numeric strings), a plain `string` does not, and a
template *expression* is typed by its own pattern in a const context or
under a template-literal contextual type. surge resolved every pattern to
`string`, so nothing written against one was ever rejected. A primitive's
apparent type is its global wrapper interface, so `string` to `String`
holds.
