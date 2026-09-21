# object-literal-union-head-basic

An object literal typed against a union is matched to the constituent it
belongs to, and may lack members of that one. tsc names those beneath the
head, which stays the relation to the union as written (TS2345 for an
argument, TS2322 otherwise) — not TS2741 against the constituent. With no
discriminant to pick a constituent the union stays the contextual type, so a
literal that fits another member as it stands is accepted: `{ a: '', b: '' }`
is a `Bar` even though only `Foo` declares both names.
