# conditional-readonly-array-infer-basic

`T extends readonly (infer U)[]` infers `U` from any array or readonly array
exactly as the mutable pattern does — tsc's `inferFromObjectTypes` relates
`Array` and `ReadonlyArray` references by their type arguments
(`isArrayType` on both sides). The pattern used to leave `U` unbound, so the
whole conditional degraded (trpc's `Serialize` goes through it for every
procedure output).
