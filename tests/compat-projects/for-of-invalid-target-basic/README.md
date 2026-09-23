# for-of-invalid-target-basic

`checkForOfStatement` checks a `for...of` (and `for await...of`) target with `checkReferenceExpression`: a target that is not a variable or property access is TS2487 over the target, parentheses included, the `for...of` counterpart of the `for...in` TS2406. Every such target is reported and the rest of the file is still checked.
