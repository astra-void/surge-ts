# mapped-type-parameter-self-constraint

A mapped type's key is declared in the mapped type's own scope, so its
constraint can name it — shadowing an outer type parameter of the same name.
tsc's `checkTypeParameter` resolves the key's base constraint to reveal
circularity: a constraint that is the key, or a union with the key in it, is
TS2313, while `keyof P` is not circular (its base constraint is the key-of
constraint). surge resolved the constraint without the key in scope, so each
of these was a false TS2304 and none was TS2313.
