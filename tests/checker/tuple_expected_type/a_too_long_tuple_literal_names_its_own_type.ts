// tsc names the source by its own widened element types; the hardcoded
// `unknown[]` this used to print named neither side truthfully.
// @surge-compare: messages
export const pair: [string, number] = ["Ada", 36, true];
