# modifier-combinations-basic

tsc's `checkGrammarModifiers` TS1243: `abstract` with `private`, `static` or
`async` (reported on the `async` modifier whichever order they come in),
`accessor` with `readonly` in either order, and `override` after `declare`.
`protected abstract` and `static async` are fine.
