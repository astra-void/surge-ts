# object-spread-operand-type-basic

An object literal's spread element was typed but never checked, so tsc's
`isValidSpreadType` rule (TS2698) had no counterpart.

What the rule accepts is not "object-shaped": tsc runs
`removeDefinitelyFalsyTypes` over the operand first, which is why the two
`undefined` cases split. `{ ...maybeObject }` is legal — the `undefined` half is
dropped and an object remains — while `{ ...undefined }` is not, because
dropping it leaves nothing. An array is an object type and spreads fine;
`unknown` does not, and a union is valid only when every surviving constituent
is. A type surge failed to model is treated as valid, so a modelling failure
cannot turn into a false TS2698.

tsc anchors the error on the spread element, not on the object literal or on the
spread argument.

The spread of a bare `null` **literal** (`{ ...null }`) is deliberately not
pinned here: the parser types a `null` literal as `any`, so the operand is
valid by this rule and no error is reported. Only the annotated form
(`declare const nu: null`) reaches it. That is a separate modelling decision
about `null`, not part of the spread rule.

`{ ...(u ?? {}) }` where `u` is `unknown` is legal, and pinning it here is what
keeps the rule honest about `??`: under `strictNullChecks` tsc expands `unknown`
to `{} | null | undefined` before taking the non-nullable half
(`getAdjustedTypeWithFacts`), so `u ?? x` is `{} | x` — an object type — while a
bare `unknown` operand still fails. The same holds when the `unknown` reaches
the operator inside a union, as an optional `input?: unknown` member does.
