# interface-call-signature-generic-binding-basic

A generic call signature written on an interface or a type alias had its type
parameters erased to the degradation sentinel at resolve time:
`resolve_function_type` keeps only their *rendering* (`type_parameter_head`),
and attaches no declaration to the handle. Every path that later wanted to bind
them — `instantiate_declared_member_signature` on the property path, the two
`?.()` arms, and the inference twin in `infer_property_call` — had nothing to
bind from, so the call returned `T[]` with `T` unresolved and assignability
suppressed the result as unknown. The call was checked; only its answer was
degraded, which is why the failure reads as silence rather than a wrong type.

The recovery already existed for one narrow spelling — a *direct* call of a
value typed by a *non-generic* interface — and was reached only from the
bare-call path. Three things were missing:

- the property path never consulted it, so `holder.wrap('a')` bound nothing;
- it refused a reference carrying type arguments and a declaration with its own
  type parameters, so `WrapIn<number>` failed on *both* paths. The declaration's
  own parameters are bound by the reference that named it, and the written
  annotation mentions them, so they are seeded alongside the ones inferred from
  the call. An inner type parameter shadows an outer one of the same name, and
  an arity mismatch binds nothing rather than something wrong;
- it refused a type alias outright, so `type WrapAlias = <T>(value: T) => T[]`
  lost its parameters everywhere.

`nested` pins the inference twin: an object literal holding a nested call of the
same shape is typed by `infer_property_call`, which read the return type
straight off the resolved handle. Without instantiation there the outer call saw
an unresolved argument and abandoned its own binding, which is what kept
tRPC's `t.router({ post: t.router({ … }) })` open even once the direct spelling
worked.

The last two declarations keep the binding honest rather than permissive: a call
that matches still type-checks, and an argument the signature refuses still
reports.
