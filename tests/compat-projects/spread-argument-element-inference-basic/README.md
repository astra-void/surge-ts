# spread-argument-element-inference-basic

`f(...xs)` supplies the *elements* of `xs`, each lined up with the position it
covers — tsc's `getSpreadArgumentType`. Inference matched the spread's own type
against the parameter instead, so a rest `...items: T[]` bound `T` to `number[]`
rather than `number`, and every ordinary argument standing beside the spread was
then rejected against the wrong element type (`rest(...numbers, 5)`).

An array literal carrying a spread was worse: the parser returned `None` for the
whole expression, so `[...xs]` and `[...xs, tail]` — the commonest way to copy or
extend an array — had no type at all and every diagnostic depending on one went
missing.

A spread contributes what iterating its operand yields, which is why a `Set`
works the same way an array does. A shape surge cannot iterate contributes
nothing rather than a guessed element.

Not pinned here: tsc also checks each element of a *tuple* spread against the
parameter it covers (`rest(...tuple)` reports the second element against the `T`
the first one fixed). surge still skips argument checking for a spread, so that
`TS2345` is a known false negative.
