# mapped-type-optionality-modifier-basic

A mapped type's explicit `+?` / `-?` optionality modifiers were unmodelled: the
parser returned `Unknown` for the whole mapped type rather than the mapping it
describes. Anything that read the mapping's members then failed — indexing it by
`keyof t` inside the same alias (`{ [k in keyof t]-?: … }[keyof t]`, ts-pattern's
`Contains`) reported `TS2538` against a key union the mapping should have had.

The three states are not a boolean: no modifier inherits the source property's
optionality, `+?` (and bare `?`) forces optional, and `-?` forces required *and*
strips `undefined` from the property type, which is what makes `Required<T>`
produce `number` rather than a required `number | undefined`.

Each case is paired with a negative control, so the mapping cannot pass by
becoming permissive: `+?` must still reject a possibly-undefined read, `-?` must
not invent absent members, and the mapped value type must stay exact.
