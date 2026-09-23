# decorator-targets-legacy

tsc's `nodeCanBeDecorated` with `experimentalDecorators`: only a class
declaration and its members with bodies can be decorated, a private-named
member cannot, and a parameter only of a constructor, method or setter with a
body in a class declaration. TS1206 goes on the first decorator of a rejected
node, and a rejected property's computed name is not checked (no TS1166), as
`checkGrammarModifiers` fails first. Abstract and `declare` fields are fine.
Paired with `decorator-targets-es`.
