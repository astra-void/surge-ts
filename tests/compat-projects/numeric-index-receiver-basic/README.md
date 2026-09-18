# numeric-index-receiver-basic

A receiver whose only index is numeric — an array, a tuple, a `string` —
cannot answer any other key, so tsc reports the access as an implicit `any`
(TS7015, silent without `noImplicitAny`). surge reported TS2322 against
`number` instead, said nothing at all for a `string` receiver, and reported
the `for (const k in xs) xs[k]` idiom the rule exists for
(`isForInVariableForNumericPropertyNames` covers an array, not only an
object with numeric property names).
