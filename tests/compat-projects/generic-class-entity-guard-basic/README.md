# generic-class-entity-guard-basic

A generic class's value side is modelled as `any` (see
`program::classes::build_class_value_symbol_with_scope`): a real static object
over it was measured to open TS2351/TS2554 across zod, trpc and ofetch. That
`any` also erases the class identity, and the identity is the whole content of
the guard this fixture pins — `is<T extends EntityClass<any>>(value: any, type:
T): value is InstanceType<T>`, where `T` is inferred from the *class* passed
alongside the tested value. `T` bound to `any`, `InstanceType<any>` came back
`any`, and the guard then either replaced the subject with `any` (silently
losing every later error in the branch) or proved nothing at all and left the
whole union standing.

A class merged with a namespace (`class Sql` + `namespace Sql { class Aliased }`)
reaches the same place from the other side: its value is the namespace object,
which carries the members but no way to construct. Both now get a constructor
surface over the class's instance type, for type-argument inference only — the
value's own type is untouched, so `new Sql()` and `Sql.Aliased` are unchanged.

`decoderOf` is the shape drizzle writes everywhere and the reason the corpus
carried 52 of these. `QueryBuilder`'s constructor pins the other half:
`is(dialect, Dialect) ? dialect : undefined` over `Dialect | DialectConfig |
undefined` assigns to two differently-typed fields, so both branches have to
select the right member rather than fall back.

`theGuardStillExcludesTheOtherMember` is the intentional error and pins the
direction: the true branch really is `Sql`, so reading `fieldAlias` there
reports — and the else branch, which is `Sql.Aliased`, does not.
