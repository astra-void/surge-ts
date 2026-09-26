# grammar-top-level-await-basic

tsc's `checkGrammarAwaitOrAwaitUsing` and `checkGrammarForInOrForOfStatement`
at the top level: in a file with no import or export an `await`, a
`for await` and an `await using` are TS1375/TS1431/TS2853, and under a
`module`/`target` pair that cannot run top-level `await` (here `es2015`) each
is also TS1378/TS1432/TS2854 — in a module file too. `await (pending)` in the
module is no error: tsc's parser reads that `await` as a name
(`isAwaitExpression`) and reparses the statement in an await context.
Inside an `async` function nothing is reported.
