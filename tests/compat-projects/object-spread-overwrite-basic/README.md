# object-spread-overwrite-basic

tsc's `checkSpreadPropOverrides` (TS2783, under `strictNullChecks`): a
property written before a spread whose type always carries it is overwritten.
The rest of `const { a, ...rest } = o` omits `a` (tsc's `getRestType`), so
spreading it back after `a` is not an overwrite — surge had typed the rest as
all of `o` and reported one.
