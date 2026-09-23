# arrow-captures-global-this-basic

In a script file, `this` inside a top-level arrow function captures
`globalThis` — TS7041 under `noImplicitThis` (tsc's `checkThisExpression`).
A top-level `this` outside an arrow, a class member's arrow and an object
method's arrow are fine; inside a function declaration it is TS2683.
