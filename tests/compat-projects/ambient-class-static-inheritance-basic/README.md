# ambient-class-static-inheritance-basic

`import { EventEmitter } from "stream"` reads a static that `@types/node`'s
`class Stream extends EventEmitter` inherits from a base bound by an import
inside the ambient module block, and that base carries the member through a
namespace merged into it (`namespace EventEmitter { export { internal as
EventEmitter } }`). surge bound each block's classes before its imports were
resolvable and before the namespace merge was applied, so a derived class had
none of its base's statics and the import was a false TS2305.

Statics are now re-merged after the namespace merge (same block) and again
once every ambient module's exports are resolved (an imported base, including
one reached through `export *`). A base surge can only model as `any` leaves
the derived static side open, so a static read through it is `any` rather than
a missing export — that is the `stream` case itself, which this fixture
therefore cannot pin beyond "no error". A namespace-merged member itself
(`Base.limit`) is still modelled permissively inside an ambient module, so
only the declared statics (`Derived.shared`, and `defaultMaxListeners` read as
a named import off the `export =` class) carry a type that the two intentional errors can pin; `export { Class }` now
exports the merged class as well.
