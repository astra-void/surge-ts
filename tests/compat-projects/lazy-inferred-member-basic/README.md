# lazy-inferred-member-basic

Types tsc reads from an expression rather than an annotation, resolved on first
read (Go's `getReturnTypeOfSignature` is lazy the same way):

- `prepareUrl`'s return comes from a body that calls a same-module helper. A
  lazily forced body used to resolve against no value table at all, so the call
  degraded and the declaration kept surge's sentinel.
- `Connection`'s unannotated field and getters were typed `any`, which silenced
  every read. tsc types a property from its initializer and a getter from its
  body's return.
