# destructuring-pattern-tuple-context

An unannotated declaration's initializer is contextually typed by the type its
binding pattern implies (tsc's `getContextualTypeForInitializerExpression` and
`getTypeFromBindingPattern`): a tuple for an array pattern, an object with the
pattern's keys for an object pattern. An array literal whose contextual type
is tuple-like — a tuple, or a type with a `"0"` property (`isTupleLikeType`) —
is a tuple (`checkArrayLiteral`), so each binding reads its own element,
nested patterns reach nested literals, a rest element reads the slice, and an
element past the literal's end is TS2493. `{ 1: e }` has no `"0"` key, so that
literal stays an array. surge typed every such literal as an array, so each
binding read the union of all elements.
