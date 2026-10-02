// An element that does not resolve stands in as `any`, which is how tsc renders
// its error type — and the length mismatch is still one diagnostic, with no
// cascade from the unresolved element.
// @surge-compare: messages
export const pair: [string, number] = ["Ada", 36, missing];
