# javascript-jsdoc-and-commonjs

A checked JavaScript file's JSDoc is its annotations: `@param`, `@returns`,
`@type` (on a variable, a class field, a parameter, a cast or a `return`),
`@template`, `@typedef` with `@property` children, `@callback`, `@this`,
`@satisfies`, `@extends` and `@import`, with the JSDoc forms `*`, `?T`, `T=`,
`...T`, `String` and `Object.<K, V>`. A JavaScript signature no JSDoc types
takes any number of arguments. CommonJS: `require` reads a module (a variable
initialized to one is an alias), `module.exports = e` is its `export =` and
`exports.x = e` or `Object.defineProperty(exports, "x", …)` one of its
exports; `module` and `exports` are in scope in such a file. An empty object
literal takes members from assignments and `Object.defineProperty`. `this` in
a plain function is an implicit `any` (TS2683). An import type
(`import("m").T`) names a module's type export. A catch variable shadows a
same-named `var` hoisted out of another block, and a misspelled `toFixed` on
a string suggests `fixed`.
