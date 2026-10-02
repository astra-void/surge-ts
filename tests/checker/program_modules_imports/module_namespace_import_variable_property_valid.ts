// @filename: user.ts
export const version: number = 1;
// @filename: index.ts
import * as ns from "./user";
let version: number = ns.version;
