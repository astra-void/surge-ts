// @filename: user.ts
export default 123;
// @filename: index.ts
import * as ns from "./user";
let version: number = ns.default;
