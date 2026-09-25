# keyof-non-public-members

A private or protected member, or a `#private` one, is no key of its class
(`getLiteralTypeFromProperty` without `includeNonPublic`): `keyof` leaves it
out, and so does every homomorphic mapped type over the class, `Partial`,
`Required`, `Readonly`, `Omit` and a user mapped type included, so an
instance is assignable to each of them.
