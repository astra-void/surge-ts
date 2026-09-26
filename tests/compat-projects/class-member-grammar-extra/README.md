# class-member-grammar-extra

Class member checks tsc makes without types: a gap between two `abstract`
overloads is TS2516 rather than TS2391 (`reportImplementationExpectedError`), an
`abstract` method with a body is TS1245 (`checkMethodDeclaration`), an accessor
named `constructor` is TS1341 (`checkAccessorDeclaration`), and constructor
overloads must agree on accessibility, reported on the declaration since a
constructor has no name (TS2385, `checkFlagAgreementBetweenOverloads`).
