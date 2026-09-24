# untyped-javascript-module-no-implicit-any

The same imports as `untyped-javascript-module-basic` with `noImplicitAny`
off: an untyped JavaScript module is silent (tsc demotes TS7016 to a
suggestion), while the `.jsx` target without `jsx` (TS6142) and the specifier
nothing answers (TS2307) still report.
