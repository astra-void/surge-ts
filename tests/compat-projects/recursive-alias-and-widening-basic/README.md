# recursive-alias-and-widening-basic

A conditional alias that recurses in *tail* position resolves by consuming its
arguments instead of reading the back-edge as a cycle, which is what makes
zustand's `Mutate` chain (and any `Reverse`-style accumulator) produce a type at
all. The indexed access `Ms["length" & keyof Ms]` in its first branch is a valid
key: an intersection that includes `keyof Ms` is assignable to it.

The widening file pins what a `let` is declared as: only a *fresh* literal type
widens, so a union read from a property keeps its members and a later assignment
narrows within them — while a literal read from a `const` still widens. A `const`
that aliases a condition (`const schema = items[i] || rest`) is narrowed by the
test on it, not only by the expression it stands for.
