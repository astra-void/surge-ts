# jsx-element-grammar-checks

`checkGrammarJsxElement` stops at an element's first grammar error: a
namespaced tag whose namespace is not an intrinsic name under a JSX transform
(TS2639), then, attribute by attribute, a repeated name (TS17001) or a value
written as an empty `{}` (TS17000).
