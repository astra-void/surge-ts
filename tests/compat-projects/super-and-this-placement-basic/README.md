# super-and-this-placement-basic

tsc's `checkSuperExpression` and `checkThisExpression` placement errors:
`super` in a class without `extends` (TS2335), a `super()` call outside a
constructor or in a function nested in one (TS2337), `super` outside a class
member or object-literal method (TS2660); `this` in a class member's computed
name (TS2465), in a namespace body (TS2331), or in an enum (TS2332). Under
`noImplicitThis`, a `this` whose container supplies none — a namespace, an
enum, a function declaration without a `this` parameter — is TS2683.
