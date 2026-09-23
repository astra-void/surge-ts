# commonjs-class-and-import-names-basic

A class named `Object` in a file emitted as CommonJS is TS2725
(`checkClassNameCollisionWithObject`), and an `import x = N.T` alias whose
target is a type may not take a predefined type name (TS2438). Aliases of a
value or a namespace are fine.
