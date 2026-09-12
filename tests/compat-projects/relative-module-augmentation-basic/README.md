# relative-module-augmentation-basic

`declare module "./generated" { … }` inside a sibling `.d.ts` — the shape
`@typescript-eslint/types` uses to hang `parent` on every AST node — was
collected but never applied.

Augmentations are filed under the module specifier written in the `declare
module` header and looked up under the specifier a consumer writes. That works
for a bare specifier, which is the same string everywhere. A relative one names
a file relative to the *augmenting* file, so no consumer outside that directory
ever writes it, and the augmentation sat in the map unused.

A relative specifier is now resolved the way an import of it is resolved
(`resolve_relative_module`, against the program's file index) and filed under
the target's canonical identity, which every import path already has in hand.
All four resolution routes — package and relative, in both `try_resolve_module`
and `try_resolve_module_export_table` — consult it.

The augmentation of a *base* interface read through a derived one
(`interface BaseNode { parent }` seen via `Identifier extends BaseNode`) is
covered by `relative-module-augmentation-heritage-basic`.
