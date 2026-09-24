# super-call-placement-basic

With `useDefineForClassFields: false`, a derived class with initialized
properties, parameter properties or private names needs `super()` as a
root-level statement (TS2401) and as the first statement touching
`this`/`super` (TS2376), per tsc's `checkConstructorDeclaration`.
