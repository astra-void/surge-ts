# builtin-global-name-conflicts-basic

A script's top-level declaration of `undefined` or `globalThis`, and a
`declare global` value named `undefined`, conflict with the built-in globals
(TS2397). Module locals and nested declarations do not.
