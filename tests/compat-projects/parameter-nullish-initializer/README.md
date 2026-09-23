# parameter-nullish-initializer

Under strictNullChecks a parameter initialized with `null` or `undefined`
keeps that type (`widenTypeInferredFromInitializer`), plus the `undefined`
its initializer makes optional: `orNull("text")` is TS2345 against
`null | undefined`. Only a variable declaration with such an initializer is
auto-typed and evolves with its assignments (`later = "text"` is fine).
