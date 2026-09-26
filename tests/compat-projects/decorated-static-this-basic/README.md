# decorated-static-this-basic

tsc's `checkThisExpression` for class property initializers. Under
`experimentalDecorators`, `this` in a static property's initializer of a
decorated class — directly or through an arrow — is TS2816
(`checkThisInStaticClassFieldInitializerInDecoratedClass`). A `function`
expression initializing a class property, or returned from a function with no
contextual return type, gets no contextual `this`, so a `this` in it is TS2683
under `noImplicitThis`.
