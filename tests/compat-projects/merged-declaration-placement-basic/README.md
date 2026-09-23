# merged-declaration-placement-basic

Merged global declarations: an instantiated namespace merging with a class or implemented function must follow it in the same file (TS2434) and may not be in another file (TS2433); a function with a body merges only with an ambient class (TS2813/TS2814); merged properties must share modifiers (TS2687); and a `var` may not hoist past a block-scoped declaration of its name (TS2481).
