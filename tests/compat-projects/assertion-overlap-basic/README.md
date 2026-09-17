> **Partly withheld.** Between object types this check stays unemitted: the comparable relation it needs is only as faithful as surge's structural expansion of a library's generic types. It is exact on these fixtures and produced **54 false positives on zod**, which was otherwise diagnostic-exact. Assertions between primitives and their literals are emitted (`primitive-assertion-overlap-basic`).
> The fixture and the analysis below are kept for whoever re-lands the rest;
> the preset is unregistered so the sweep does not compare it.

`x as T` was never checked, so TS2352 had no emission point.

The rule rejects a conversion only when the two types fail to overlap in
*either* direction: an upcast (`dog as Animal`) and a downcast
(`animal as Dog`) are both legitimate, and only a conversion between unrelated
types is the mistake. Both directions are pinned, because implementing this as
one-directional assignability looks right and is wrong.

The case that separates a faithful port from a plausible one is `ok9`:
`lit as "z"` on a `"a" | "b"` source is **accepted**. tsc applies
`getBaseTypeOfLiteralType` to the source first, so the comparison is `string`
against `"z"`, which overlaps. Without that widening this reads as an obvious
error.

`unknown`, `any` and a type surge failed to model relate to everything, so none
of them can be the mistake — `obj as unknown as string`, the documented escape
hatch, stays legal.
