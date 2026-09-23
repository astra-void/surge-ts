# global-augmentation-class-alias-values

A `declare global { … }` block in a module merges its declarations into the
global scope (the binder's global augmentation, merged with
`mergeSymbolTable(globals, …)`), every meaning included: a class there is a
global constructor value as well as a global type, and an
`export import X = N.M` there is a global alias of the entity, whose root
resolves from the block outwards — here to a namespace local to the declaring
module.

`OnlyAType` is an interface, so using it as a value is still TS2693, and the
class's and the alias's own types are checked where they are used.
