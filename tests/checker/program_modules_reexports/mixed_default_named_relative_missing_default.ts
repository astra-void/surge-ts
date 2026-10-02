// @filename: thing.ts
export function helper(): number { return 1; }
// @filename: index.ts
import DefaultThing, { helper } from "./thing";
let count: number = helper();
