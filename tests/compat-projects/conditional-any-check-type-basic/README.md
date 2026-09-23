# conditional-any-check-type-basic

tsc's `getConditionalType` (checker.go) treats an `any` check type as matching
every extends type: the result is the true branch, instantiated with what
inference from `any` binds its `infer` captures to, together with the false
branch — unless the extends type is itself `any` or `unknown`, where the
definitely-true test answers with the true branch alone. `Get<any>` is
`"no" | Box<unknown>`, and an immediately nested conditional contributes all of
its branches.

Inference from `any` reaches only naked captures (`T extends infer U` binds
`any`); every other capture has no candidate and takes its constraint: the
written one (`infer U extends string`), or the one its position implies
(`getInferredTypeParameterConstraint`): `unknown[]` for a rest parameter, the
constraint of the type parameter it fills in `Fn<infer D, infer K>` (`Key`), and
`unknown` otherwise.

`IsAny<T> = 0 extends 1 & T` and `[T] extends [never]` have no `any` check type
and keep deciding by assignability. Every `Symbol()` initializer and the `bad*`
constants are the intentional errors; all are `tsc` errors too.
