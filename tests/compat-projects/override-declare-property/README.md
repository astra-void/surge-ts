# override-declare-property

tsc's `checkMembersForOverrideModifier` skips a class member with the
`declare` modifier, so under `noImplicitOverride` a `declare` property that
narrows a base property's type (drizzle's `declare readonly session:
SQLiteD1Session<…>`) needs no `override`, while an ordinary redeclaration of
a base property (`name`) is still TS4114.
