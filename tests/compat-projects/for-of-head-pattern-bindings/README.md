# for-of-head-pattern-bindings

A numeric or quoted key in an object binding pattern binds its element like
any other key: tsc types `{ 0: key }` by indexing the source with the key's
literal type (`getLiteralTypeFromPropertyName`), which for a tuple is the
element. surge's parser dropped numeric keys (the binding was never declared,
so every use was TS2304) and dropped quoted keys in variable declarations, and
a numeric key read no tuple element. `wrong` and `read` use the tuples'
elements at the wrong type, which are the two intentional errors.
