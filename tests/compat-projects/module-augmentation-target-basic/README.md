# module-augmentation-target-basic

A `declare module "m"` in a module file augments `m`: an unresolvable name is TS2664, and a module whose `export =` target has no namespace meaning (a class, function, variable or interface, but not a namespace-merged class or an enum) is TS2671 (tsc's `mergeModuleAugmentation`). An ambient `declare module` of the same name is found first.
