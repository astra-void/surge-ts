# global-augmentation-fragment-heritage-scope

A `declare global` block reopens a global interface, and each fragment of the
merged interface resolves where it is written, as tsc resolves every
declaration in its own lexical scope.

- `events.d.ts` is a module. Its `interface Emitter extends __NodeEmitter {}`
  names a module-local alias, so the heritage clause must resolve under the
  module's scope and not under the scope of `globals.d.ts`, where the merged
  declaration starts. This is the `@types/node` shape of
  `interface EventTarget extends __EventTarget {}`.
- `url.d.ts` reopens `Address` from a `global` block nested in
  `declare module "fakeurl"`. There the base is an import written inside that
  block (`import { Address as _Address } from "fakeurl"`), which is the shape of
  `interface URL extends _URL {}` in `@types/node`.
- `events.d.ts` also declares a module-local `Listener`. The global fragment's
  `on(listener: Listener)` must keep meaning the global `Listener`.

The two intentional errors bind inherited members to the wrong type, so they
only report when each base resolves.
