# inference-any-candidate-supertype

tsc's `inferFromTypes` records an `any` argument as an inference candidate like
any other, and `getCommonSupertype` then picks the candidate no later one is a
supertype of. Every type is a subtype of `any`, and `any` is a subtype of
nothing but itself (`isSimpleTypeRelatedTo` admits an `any` source only under
assignability), so `same(7, anything, 4)` binds `T` to `any` whichever position
the `any` argument takes. surge used to keep only a leading `any` and otherwise
settled on `number`.

The two intentional errors — `numbersAreNumber` and `each("", 1, 2)` — are
the ordinary no-`any` outcome and are tsc errors too.
