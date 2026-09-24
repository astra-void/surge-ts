# enum-reverse-mapping-basic

A numeric enum's object carries a reverse mapping: tsc gives it the readonly
index signature `[n: number]: string` (`enumNumberIndexInfo`), so
`Direction[1]` reads `string` rather than an implicit `any`. The rule is the
one `resolveAnonymousTypeMembers` applies — the enum has no members, or at
least one member is numeric — so a string-only enum has no reverse mapping and
`Tone[0]` stays TS7053, while an enum mixing the two (or merged across
declarations into one) has it. A string key the index cannot answer is TS7015.

`keyof typeof Direction` does not include `number`: `getLiteralTypeFromProperties`
skips the reverse mapping.

Not covered here: a `const enum` indexed by anything but a string literal is
TS2476 and the error type (no TS7053 follows), and a write through the reverse
mapping is TS2542, which surge does not report yet.
