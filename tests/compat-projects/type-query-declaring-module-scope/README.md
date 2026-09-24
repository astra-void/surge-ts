# type-query-declaring-module-scope

A `typeof x` inside a declaration names `x` in the declaring module's scope,
whichever file reads the declaration: `Thing.n` of `foo.d.ts` is `number`
(its own `x`), not the importing file's `x: Thing`, through a named and a
namespace import alike.
