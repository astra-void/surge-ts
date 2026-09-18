# declaration-merging-values-basic

An `enum` or `namespace` written twice in one scope is one declaration: tsc
merges the bodies. surge lowered each block separately, so the members of one
block silently replaced (enum) or were dropped by (namespace) the other, and
reading a member off the losing block was a false TS2339.
