# conditional-infer-interface-pattern-basic

A conditional type's `infer` pattern naming an interface the check type is not
an instance of captures member by member (`inferFromObjectTypes` →
`inferFromProperties`): `StandardSchemaV1<infer I, infer O>` reads `I` and `O`
through the schema's `'~standard'` property. surge expanded only alias patterns,
so the captures stayed unbound (tRPC's `inferParser`).
