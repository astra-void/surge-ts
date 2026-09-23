# satisfies-argument-elaboration

tsc relates a call argument through `satisfies` and parentheses
(`getEffectiveCheckNode`): a mismatched object or array literal inside is
elaborated at the member that does not fit (TS2322), as the bare literal
would be, while any other expression is reported as the argument (TS2345).
A literal `true` checked against `{ a: boolean }` keeps its literal type, so
it still fits `{ a: true }`.
