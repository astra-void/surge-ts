# shorthand-property-scope-basic

An unresolved shorthand property is `TS18004`, not a plain missing name: the
property has no initializer to fall back on, and tsc's message says so. A
spelling suggestion still wins (`suggestion` reports `TS2552`), exactly as it
does for any other reference.

`excess` pins the interplay the same change uncovered: an excess property's
*value* is an expression with errors of its own, and tsc reports them alongside
the excess report. surge reported only the excess one and stopped.
