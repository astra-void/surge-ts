# import-helpers-missing-tslib

`importHelpers` with no `tslib` to resolve: a file whose emit calls a helper
reports TS2354 once, at the first construct the checker asks `tslib` for —
a namespace import, a re-exported default, a decorator, a private field, or
(ahead of the object rest in a deferred arrow body) an async function. A
named `default` import of a module that does not resolve asks for nothing,
and a script file is never checked for helpers.
