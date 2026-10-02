// @filename: user.ts
export type Name = string;
// @filename: index.ts
import type { Name } from "./user";
let name: Name = "Ada";
