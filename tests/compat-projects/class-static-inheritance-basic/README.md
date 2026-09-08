# class-static-inheritance-basic

`class D extends B {}` makes every static of `B` reachable as `D.x`. surge built
a class's static side from its own members alone, so every inherited static was
a false `TS2339` — including the ones a `.d.ts` reaches through a namespace
merged into the base class.

The base's static side is a *value*, not part of the instance-side interface a
class is bound as, so it is read from the value table the class is being bound
into rather than from `ctx.symbols`, which does not have it yet at binding time.
A base that is not in that scope contributes nothing, which is what surge did
for every base before.

`prototype` is the one inherited entry that must not survive: `Leaf.prototype`
is a `Leaf`, not a `Base`. The two intentional errors pin that the inherited
member keeps its real type and that a name no class declares still reports.

The statics here are annotated on purpose. A static with an initializer and no
annotation (`static origin = 'base'`) does not get its type inferred yet — a
separate, still-open gap that would otherwise mask what this fixture pins.
