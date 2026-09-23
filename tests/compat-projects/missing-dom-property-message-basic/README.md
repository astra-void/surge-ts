# missing-dom-property-message-basic

tsc's `reportNonexistentProperty` ends in TS2812 instead of TS2339 when
`containerSeemsToBeEmptyDomElement` holds: `compilerOptions.lib` does not list
`dom`, every constituent of the receiver is declared as an `EventTarget`,
`Node`, `Element` or `HTML…Element` interface or class, and the receiver is an
empty object type. This project's `lib` has no DOM, so empty interfaces with
those names, global or module-local, get the hint.

The TS2339 cases are receivers that miss one condition: a name that only
starts with `HTML`, a `Node` with a member, an alias of a type literal (whose
symbol is the literal's), an intersection with a type literal, and `object`.
