# namespace-body-basic

tsc's `checkModuleDeclaration` checks a namespace body as ordinary source
elements; surge skipped it entirely, so nothing inside `namespace`/`module`
blocks was ever checked. The body is its own block scope (a `const` there
shadows an enclosing one rather than redeclaring it), later declarations and
other blocks' exports are visible (merged blocks, nested ones included), bare
type names resolve to the namespace's members, and a `declare namespace` is
ambient (its classes have no initializers to check).
