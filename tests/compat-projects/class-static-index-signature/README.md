# class-static-index-signature

A `static` index signature belongs to the constructor type (tsc
`resolveAnonymousTypeMembers` reads the class symbol's `__index` member), so
`Registry["anything"]`, `Registry.other` and `Registry[2]` read the static
string and number index types. An instance index signature does not apply to
the constructor: a class without a static one still reports TS2339 for a
missing static property.
