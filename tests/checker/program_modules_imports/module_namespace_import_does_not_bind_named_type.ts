// @filename: user.ts
export interface User { name: string; }
// @filename: index.ts
import * as ns from "./user";
let value: User = { name: "Ada" };
