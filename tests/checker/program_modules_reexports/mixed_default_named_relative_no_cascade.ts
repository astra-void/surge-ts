// @filename: thing.ts
// The default export is missing (TS1192), but the named `helper` binds and
// the unknown default binding must not cascade into TS2304 on `DefaultThing()`.
export function helper(): number { return 1; }
// @filename: index.ts
import DefaultThing, { helper } from "./thing";
let count: number = helper();
let made = DefaultThing();
