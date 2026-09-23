# mixin-static-members-basic

tsc's `resolveAnonymousTypeMembers` builds a class's static side from its own
statics plus `getPropertiesOfType(getBaseConstructorTypeOfClass(C))` whenever
the base constructor type is an object, an intersection or a type variable.
The base constructor type is the type of the `extends` expression, so a class
extending a mixin call such as `Mix(A, B)` (typed `typeof A & typeof B`)
inherits the statics of every constituent, and one extending `Id(A)` those of
`A`.

surge's parser dropped any `extends` expression that is not a (dotted) name,
so such a class had only its own statics and every inherited read was a false
TS2339. The intentional errors are reads of statics no constituent declares
and a mistyped assignment from an inherited static.
