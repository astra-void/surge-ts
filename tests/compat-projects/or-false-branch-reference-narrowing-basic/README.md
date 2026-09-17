# or-false-branch-reference-narrowing-basic

Every operand of an `||` is false where the whole test is, so after
`if (!s?.program || !s.map) return;` both `s.program` and `s.map` are truthy.
The member-level truthy guards were collected only for the true branch of an
`&&`; the false branch of an `||` — and a single `!` flipping the branch — was
missing. With an optional chain in the condition, `s` itself is re-narrowed as
a guarded operand, which dropped the `s.program` narrowing and reported a false
TS18048. The last case pins that a member the condition does not test stays
possibly undefined.
