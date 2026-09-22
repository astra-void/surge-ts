# function-expression-this-basic

TS2683 for a function expression, where tsc's `getContextualThisParameterType`
finds no `this`: an unannotated variable (or one annotated with a function type
without a `this` parameter), an IIFE callee, an array or conditional in such a
place, or the `return` of a function declaration without a return type. An
object-literal member and a member assignment give `this` the object, and a
`this:` parameter in the annotation types it.
