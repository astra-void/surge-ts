# catch-yield-await-placement-basic

tsc's placement rules for bindings and operators that belong to one
construct: a catch variable redeclared by a block-scoped variable in its block
(TS2492) and a catch annotation that is not `any`/`unknown` (TS1196, anchored at
the annotation); `yield` outside a generator (TS1163) or in a parameter
initializer (TS2523); `await` in a parameter initializer (TS2524) or a class
static block (TS18037); and `for await` in a non-async function (TS1103).
