# this-container-rules-basic

The checks that read tsc's `getThisContainer`: a `this` type outside a
non-static class or interface member is TS2526 (a method's type literal is not
the member itself); `new.target` outside a function declaration, function
expression, or constructor is TS17013; `infer` outside a conditional type's
`extends` clause is TS1338; and a predefined type name cannot name a user
type (TS2427, TS2457, TS2414, TS2431, TS2368).
