# comma-operator-side-effects

tsc reports a discarded comma operand only when `isSideEffectFree` holds of it
and the comma is not an indirect call. `isSideEffectFree` looks through
parentheses alone, so an `as` cast counts as having effects, and it treats
`||`, `&&` and `??` as binary expressions free of effects when both sides
are. `isIndirectCall` exempts `(0, x.f)(…)`, a tagged `(0, x.f)` and
`(0, eval)(…)`, but not an uncalled `(0, x.f)`, one wrapped in a second pair
of parentheses, or one led by anything but `0`.
