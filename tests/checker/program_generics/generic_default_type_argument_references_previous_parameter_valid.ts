// @filename: box.ts
type Pair<A, B = A> = [A, B];
// @filename: index.ts
let pair: Pair<string> = ["Ada", "Ada"];
