# global-augmentation-entity-alias-types

`export import x = A.y` inside `declare global` merges an alias into the
globals, and an alias has every meaning of its entity. `A.y` is a `const` and
an `interface`, so `x` is a global value and a global type; `n = A.N` over a
namespace of interfaces makes `n.I` a global type and `n` alone a namespace
(TS2709 as a type). surge published only the alias's value, so `x` as a type
was TS2749 and `n.I` resolved to nothing.
