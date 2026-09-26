# grammar-using-declarations-basic

tsc's `checkGrammarVariableDeclaration` and
`checkGrammarVariableDeclarationList` for `using` and `await using`: a
binding pattern is TS1492 (reported before the missing-initializer rule), a
declaration without an initializer is TS1155 named by its keyword (a
`for...of` head is initialized by the loop), one written directly in a
`case`/`default` clause is TS1547/TS1548, and an `await using` must sit where
an `await` could (`checkGrammarAwaitOrAwaitUsing`): TS18054 in a class static
block, TS2852 in a function that is not `async`.

The last line pins that the `const` form of TS1155 keeps its wording now that
the message takes the keyword as an argument.
