# for-of-es5-lib-basic

Without a global Iterable (lib ES5 only) tsc falls back to isArrayLikeType,
and a for...of operand that is neither an array nor a string is TS2495
instead of TS2488.
