# global-this-script-globals

A script's `var`s, functions and instantiated namespaces are members of
`typeof globalThis`, so reading them through `globalThis` — by property,
element or indexed-access type — is fine. Its block-scoped globals (`let`,
`const`, classes and enums) are not members: tsc reports them as TS2339
(`checkPropertyAccessExpression`, and `getPropertyTypeForIndexType` for an
element access, whatever noImplicitAny says). Any other name is TS7017 under
noImplicitAny.
