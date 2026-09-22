# union-excess-property-basic

An object literal against a union target. tsc first discriminates the target
by the literal's own unit-typed properties
(`discriminateTypeByDiscriminableItems`): with `kind: "one"` written the
literal is checked against that member alone, so a property only the *other*
member declares is excess and the message names the member. Undiscriminated,
a property is excess when no member knows it (`isKnownProperty` over the
union) and the message names the union. surge checked excess properties only
when exactly one member declared any written property — and named that member
— so a literal touching two members, and every discriminated literal, went
unchecked. A member that knows every property (`any`, an index signature)
makes the union no excess-property target at all.
