# generator-grammar-and-iteration-checks

Generator declarations and their iteration protocol: an overload signature or
an abstract member cannot be a generator (TS1222), a constructor cannot be one
(TS1368), `for await` is not allowed in a class static block (TS18038), a
generator cannot be annotated `void` (TS2505), `for await` and `yield*` need an
async or sync iterator (TS2504, TS2488), `yield` evaluates to the annotation's
`TNext` and `yield*` to the delegate's return type, and an unannotated
generator's `yield` whose value is used without context is TS7057.
