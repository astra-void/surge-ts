# enum-used-before-declaration-basic

tsc's `checkResolvedBlockScopedVariable` for an enum: a regular enum read by
module-level code that runs before the declaration is TS2450, as a class is
TS2449. A `const enum` has no runtime binding (outside `isolatedModules`), an
ambient enum no evaluation, and a read inside a function runs later, so none
of those report.
