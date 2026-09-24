# union-call-signatures-basic

tsc calls a union type through `getUnionSignatures` (checker.go): for each
signature of each constituent it looks in every other constituent for a
matching one (`findMatchingSignatures` — identical, or partially: a
constituent that requires no more arguments and whose parameter types at the
signature's positions are supertypes). A signature matched everywhere keeps
its own parameters and returns the union of the matched return types, so
`{ (...a: string[]): string } | { (a: string, b: string): number }` takes
exactly two arguments. Only when no signature matches, and at most one
constituent is overloaded, are the signatures combined position by position
(`combineUnionOrIntersectionParameters`): each position intersects what the
constituents take there, a rest parameter reading its element type, so
`((...objs: { x: number }[]) => number) | ((...objs: { y: number }[]) => number)`
accepts `{ x: 0, y: 0 }`. Overloads in more than one constituent with nothing
in common leave the union uncallable (TS2349).

A method read off a union receiver (`z.f(…)`) is called through the same
signatures, and a combined `never` parameter rejects every argument that is
not `never`.

Every error below is also a `tsc` error.
