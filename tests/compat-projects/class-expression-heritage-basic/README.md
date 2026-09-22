# class-expression-heritage-basic

`class X extends mixin(Base)`: the `extends` clause is an arbitrary
expression, and tsc's `getBaseConstructorTypeOfClass` takes the base from its
construct signatures. surge's parser kept a heritage clause only when it was
a (qualified) name and dropped any other expression, so the class looked like
it had no base at all: every inherited member was a false TS2339, on reads
through an instance and through `this`. The base is not modelled — the
instance is left open, as it is for an `any` base — so a member the class
declares itself is still checked, and a class with no base stays closed.
