# javascript-object-literals-open-ended

tsc's `isJSLiteralType`: without noImplicitAny an object literal written in a
JavaScript file is open-ended, so a member it does not declare reads as
`any` wherever the object is used — through a variable, a nested property, a
function's return or a class field — and TypeScript code may add members to
it freely. Paired with `javascript-object-literals-no-implicit-any`.
