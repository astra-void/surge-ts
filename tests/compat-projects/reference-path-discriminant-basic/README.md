# reference-path-discriminant-basic

Three ways a discriminant test on a property reference went unnarrowed, each a
false TS2339 on the guarded read:

- `w.thing!.name === "x"` — tsc's `isMatchingReference` looks through `!`;
  surge's discriminant arms matched only a bare reference.
- `w.thing && w.thing.name === "x"` (and the `||` early-return form) — inside
  a logical chain the statement path narrows by reference guards only, and a
  literal test distributed over a union kept the members whose leaf can never
  equal the literal. The nested-`if` and ternary spellings already worked.
- `config.works` where `works` exists only through the string index
  signature — still a narrowable reference for `typeof`, truthiness and
  nullish tests.

Each function also reads the member the guard rules out, so the narrowing is
proved rather than silent.

The project sets `noPropertyAccessFromIndexSignature`: a slot narrowed in a
branch still comes from the index signature, so the dotted read inside the
branch keeps its TS4111 (the narrowed binding records the slot as a property;
the declared type is what answers where it comes from).
