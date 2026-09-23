# modifier-ambient-and-async-placement-basic

tsc's `checkGrammarModifiers`: `async` on a declaration that is or sits in an
ambient context (TS1040, on whichever of `async`/`declare` comes second, and
on `async` inside `declare namespace`), an accessibility modifier or `static`
on a module or namespace element (TS1044), and `async` on anything but a
function or method (TS1042) — including every modifier on an object literal
member, where a method carrying more than `async` is also TS1184.
