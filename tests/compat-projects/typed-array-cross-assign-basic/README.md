# typed-array-cross-assign-basic

The lib's typed arrays differ only in `[Symbol.toStringTag]`, a string
literal member, and that is enough for tsc to keep them apart. surge
suppresses a comparison when either side carries a member it could not
model, and every typed array does, so no cross-assignment was reported.
A disagreement between two unit-typed members is definite whatever else
is unmodelled, so it reports through that suppression.
