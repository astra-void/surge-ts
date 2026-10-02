// @surge-compare: spans
// @filename: user.ts
export const getName: string = "Ada";
// @filename: index.ts
// tsc: the module exports `getName`, so it suggests a named import.
import getName from "./user";
