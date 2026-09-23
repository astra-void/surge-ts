# optional-chain-truthy-contained-reference-basic

tsc's `optionalChainContainsReference` (`narrowTypeByTruthiness`): a truthy
optional chain proves every reference it reads through non-nullish. surge
narrowed the chain's path by rewriting the receiver, which cannot test `shape`
on a union member whose `error` is `null`, so `mutation.error` stayed nullable
inside `if (mutation.error?.shape)`. A non-optional chain proves nothing and
keeps reporting.
